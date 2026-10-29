//! Kleine ini-helpers voor game-configbestanden (Unreal-stijl): één sleutel lezen, zetten of weghalen
//! in een sectie, zonder de rest van het bestand te veranderen. Houdt UTF-16 en alleen-lezen in stand.

use crate::error::AppResult;
use std::path::Path;

/// Leest het bestand. Geeft de tekst en of het UTF-16 (met BOM) was.
pub fn read_text(path: &Path) -> AppResult<(String, bool)> {
    let bytes = std::fs::read(path)?;
    if bytes.len() >= 2 && bytes[0] == 0xFF && bytes[1] == 0xFE {
        let u16s: Vec<u16> = bytes[2..].chunks_exact(2).map(|c| u16::from_le_bytes([c[0], c[1]])).collect();
        return Ok((String::from_utf16_lossy(&u16s), true));
    }
    let s = String::from_utf8_lossy(&bytes).to_string();
    Ok((s.trim_start_matches('\u{feff}').to_string(), false))
}

/// Schrijft het bestand terug in dezelfde codering. Een alleen-lezen-bestand blijft alleen-lezen.
pub fn write_text(path: &Path, text: &str, utf16: bool) -> AppResult<()> {
    let bytes: Vec<u8> = if utf16 {
        let mut b = vec![0xFF, 0xFE];
        for u in text.encode_utf16() {
            b.extend_from_slice(&u.to_le_bytes());
        }
        b
    } else {
        text.as_bytes().to_vec()
    };
    let was_readonly = std::fs::metadata(path).map(|m| m.permissions().readonly()).unwrap_or(false);
    if was_readonly {
        let mut p = std::fs::metadata(path)?.permissions();
        #[allow(clippy::permissions_set_readonly_false)]
        p.set_readonly(false);
        std::fs::set_permissions(path, p)?;
    }
    std::fs::write(path, bytes)?;
    if was_readonly {
        let mut p = std::fs::metadata(path)?.permissions();
        p.set_readonly(true);
        std::fs::set_permissions(path, p)?;
    }
    Ok(())
}

pub fn value(text: &str, section: &str, key: &str) -> Option<String> {
    let mut in_section = false;
    for line in text.lines() {
        let t = line.trim();
        if t.starts_with('[') {
            in_section = t.eq_ignore_ascii_case(section);
            continue;
        }
        if in_section {
            if let Some((k, v)) = t.split_once('=') {
                if k.trim().eq_ignore_ascii_case(key) {
                    return Some(v.trim().to_string());
                }
            }
        }
    }
    None
}

fn newline(text: &str) -> &'static str {
    if text.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    }
}

/// Zet een sleutel in een sectie. Voegt de sleutel (en zo nodig de sectie) toe als hij ontbreekt.
pub fn set(text: &mut String, section: &str, key: &str, value: &str) {
    let nl = newline(text);
    let mut lines: Vec<String> = text.split(nl).map(|s| s.to_string()).collect();
    let mut section_start: Option<usize> = None;
    let mut in_section = false;
    for (i, line) in lines.iter_mut().enumerate() {
        let t = line.trim();
        if t.starts_with('[') {
            in_section = t.eq_ignore_ascii_case(section);
            if in_section {
                section_start = Some(i);
            }
            continue;
        }
        if in_section {
            if let Some((k, _)) = t.split_once('=') {
                if k.trim().eq_ignore_ascii_case(key) {
                    *line = format!("{key}={value}");
                    *text = lines.join(nl);
                    return;
                }
            }
        }
    }
    match section_start {
        Some(i) => lines.insert(i + 1, format!("{key}={value}")),
        None => {
            while lines.last().is_some_and(|l| l.trim().is_empty()) {
                lines.pop();
            }
            lines.push(String::new());
            lines.push(section.to_string());
            lines.push(format!("{key}={value}"));
            lines.push(String::new());
        }
    }
    *text = lines.join(nl);
}

/// Haalt een sleutel uit een sectie weg (als hij er staat).
pub fn remove(text: &mut String, section: &str, key: &str) {
    let nl = newline(text);
    let mut in_section = false;
    let kept: Vec<&str> = text
        .split(nl)
        .filter(|line| {
            let t = line.trim();
            if t.starts_with('[') {
                in_section = t.eq_ignore_ascii_case(section);
                return true;
            }
            !(in_section && t.split_once('=').is_some_and(|(k, _)| k.trim().eq_ignore_ascii_case(key)))
        })
        .collect();
    *text = kept.join(nl);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_and_remove_keep_other_lines() {
        let mut t = "[A]\r\nx=1\r\n\r\n[B]\r\ny=2\r\n".to_string();
        set(&mut t, "[A]", "x", "5");
        set(&mut t, "[B]", "z", "3");
        set(&mut t, "[C]", "k", "v");
        assert_eq!(value(&t, "[A]", "x").as_deref(), Some("5"));
        assert_eq!(value(&t, "[B]", "z").as_deref(), Some("3"));
        assert_eq!(value(&t, "[C]", "k").as_deref(), Some("v"));
        remove(&mut t, "[B]", "z");
        assert_eq!(value(&t, "[B]", "z"), None);
        assert_eq!(value(&t, "[B]", "y").as_deref(), Some("2"));
        assert!(t.contains("\r\n"));
    }
}
