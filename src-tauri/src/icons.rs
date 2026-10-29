//! Pictogrammen en vriendelijke namen van programma's (.exe), voor de lijst met achtergrond-apps
//! en als reserve-afbeelding bij games.

use crate::winapi::wide;
use base64::Engine;
use std::ffi::c_void;
use std::ptr::null_mut;
use windows_sys::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS,
};
use windows_sys::Win32::Storage::FileSystem::{GetFileVersionInfoSizeW, GetFileVersionInfoW, VerQueryValueW};
use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, PrivateExtractIconsW, HICON, ICONINFO};

/// Haalt het pictogram uit een exe en geeft het als PNG-data-URL terug.
pub fn exe_icon_data_url(path: &str, size: i32) -> Option<String> {
    let (dim, rgba) = extract_rgba(path, size).map(|r| (size, r)).or_else(|| shell_icon_rgba(path))?;
    let mut png_bytes: Vec<u8> = Vec::new();
    {
        let mut enc = png::Encoder::new(&mut png_bytes, dim as u32, dim as u32);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut writer = enc.write_header().ok()?;
        writer.write_image_data(&rgba).ok()?;
    }
    Some(format!("data:image/png;base64,{}", base64::engine::general_purpose::STANDARD.encode(&png_bytes)))
}

fn extract_rgba(path: &str, size: i32) -> Option<Vec<u8>> {
    let wpath = wide(path);
    let mut hicon: HICON = null_mut();
    let mut id: u32 = 0;
    let n = unsafe { PrivateExtractIconsW(wpath.as_ptr(), 0, size, size, &mut hicon, &mut id, 1, 0) };
    if n == 0 || n == u32::MAX || hicon.is_null() {
        return None;
    }
    let result = unsafe { icon_to_rgba(hicon, size) };
    unsafe {
        DestroyIcon(hicon);
    }
    result
}

/// Reserve via de Verkenner (werkt ook voor snelkoppelingen): 32x32.
fn shell_icon_rgba(path: &str) -> Option<(i32, Vec<u8>)> {
    use windows_sys::Win32::UI::Shell::{SHGetFileInfoW, SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON};
    let wpath = wide(path);
    let mut sfi: SHFILEINFOW = unsafe { std::mem::zeroed() };
    let r = unsafe { SHGetFileInfoW(wpath.as_ptr(), 0, &mut sfi, std::mem::size_of::<SHFILEINFOW>() as u32, SHGFI_ICON | SHGFI_LARGEICON) };
    if r == 0 || sfi.hIcon.is_null() {
        return None;
    }
    let out = unsafe { icon_to_rgba(sfi.hIcon, 32) };
    unsafe {
        DestroyIcon(sfi.hIcon);
    }
    out.map(|v| (32, v))
}

unsafe fn icon_to_rgba(hicon: HICON, size: i32) -> Option<Vec<u8>> {
    let mut info: ICONINFO = std::mem::zeroed();
    if GetIconInfo(hicon, &mut info) == 0 {
        return None;
    }
    let cleanup = |info: &ICONINFO| {
        if !info.hbmColor.is_null() {
            DeleteObject(info.hbmColor as _);
        }
        if !info.hbmMask.is_null() {
            DeleteObject(info.hbmMask as _);
        }
    };
    if info.hbmColor.is_null() {
        cleanup(&info);
        return None;
    }
    let mut bm: BITMAP = std::mem::zeroed();
    if GetObjectW(info.hbmColor as _, std::mem::size_of::<BITMAP>() as i32, &mut bm as *mut _ as *mut c_void) == 0 {
        cleanup(&info);
        return None;
    }
    let (w, h) = (bm.bmWidth, bm.bmHeight);
    if w != size || h != size {
        // Onverwachte maat: toch uitlezen in de echte maat is complexer; overslaan.
        cleanup(&info);
        return None;
    }
    let hdc = CreateCompatibleDC(null_mut());
    let mut bmi: BITMAPINFO = std::mem::zeroed();
    bmi.bmiHeader = BITMAPINFOHEADER {
        biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
        biWidth: w,
        biHeight: -h, // van boven naar beneden
        biPlanes: 1,
        biBitCount: 32,
        biCompression: BI_RGB,
        ..std::mem::zeroed()
    };
    let mut buf = vec![0u8; (w * h * 4) as usize];
    let lines = GetDIBits(hdc, info.hbmColor, 0, h as u32, buf.as_mut_ptr() as *mut c_void, &mut bmi, DIB_RGB_COLORS);
    let mut has_alpha = false;
    if lines != 0 {
        for px in buf.chunks_exact_mut(4) {
            px.swap(0, 2); // BGRA -> RGBA
            if px[3] != 0 {
                has_alpha = true;
            }
        }
        // Oude pictogrammen zonder alfakanaal: gebruik het masker.
        if !has_alpha && !info.hbmMask.is_null() {
            let mut mask = vec![0u8; (w * h * 4) as usize];
            let mut mbmi = bmi;
            if GetDIBits(hdc, info.hbmMask, 0, h as u32, mask.as_mut_ptr() as *mut c_void, &mut mbmi, DIB_RGB_COLORS) != 0 {
                for (px, m) in buf.chunks_exact_mut(4).zip(mask.chunks_exact(4)) {
                    px[3] = if m[0] == 0 { 255 } else { 0 };
                }
            } else {
                for px in buf.chunks_exact_mut(4) {
                    px[3] = 255;
                }
            }
        }
    }
    DeleteDC(hdc);
    cleanup(&info);
    if lines == 0 {
        None
    } else {
        Some(buf)
    }
}

/// Leest FileDescription (of ProductName) uit de versie-informatie van een exe.
pub fn file_description(path: &str) -> Option<String> {
    let wpath = wide(path);
    let mut handle: u32 = 0;
    let size = unsafe { GetFileVersionInfoSizeW(wpath.as_ptr(), &mut handle) };
    if size == 0 {
        return None;
    }
    let mut data = vec![0u8; size as usize];
    if unsafe { GetFileVersionInfoW(wpath.as_ptr(), 0, size, data.as_mut_ptr() as *mut c_void) } == 0 {
        return None;
    }
    // Taal en codepagina's opvragen.
    let mut ptr: *mut c_void = null_mut();
    let mut len: u32 = 0;
    let sub = wide("\\VarFileInfo\\Translation");
    let mut pairs: Vec<(u16, u16)> = vec![];
    if unsafe { VerQueryValueW(data.as_ptr() as *const c_void, sub.as_ptr(), &mut ptr, &mut len) } != 0 && !ptr.is_null() {
        let count = len as usize / 4;
        let slice = unsafe { std::slice::from_raw_parts(ptr as *const u16, count * 2) };
        for i in 0..count {
            pairs.push((slice[i * 2], slice[i * 2 + 1]));
        }
    }
    pairs.push((0x0409, 0x04B0));
    pairs.push((0x0409, 0x04E4));
    for field in ["FileDescription", "ProductName"] {
        for (lang, cp) in &pairs {
            let q = wide(&format!("\\StringFileInfo\\{lang:04x}{cp:04x}\\{field}"));
            let mut p: *mut c_void = null_mut();
            let mut l: u32 = 0;
            if unsafe { VerQueryValueW(data.as_ptr() as *const c_void, q.as_ptr(), &mut p, &mut l) } != 0 && !p.is_null() && l > 1 {
                let s = unsafe { std::slice::from_raw_parts(p as *const u16, l as usize) };
                let end = s.iter().position(|&c| c == 0).unwrap_or(s.len());
                let text = String::from_utf16_lossy(&s[..end]).trim().to_string();
                if !text.is_empty() {
                    return Some(text);
                }
            }
        }
    }
    None
}
