//! Dunne laag bovenop `winreg` met een serialiseerbaar waardetype, zodat het logboek oude waarden
//! exact kan bewaren en terugzetten.

use crate::error::{map_access_denied, AppError, AppResult};
use serde::{Deserialize, Serialize};
use winreg::enums::*;
use winreg::{RegKey, RegValue};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum Hive {
    Hkcu,
    Hklm,
}

impl Hive {
    pub fn root(self) -> RegKey {
        match self {
            Hive::Hkcu => RegKey::predef(HKEY_CURRENT_USER),
            Hive::Hklm => RegKey::predef(HKEY_LOCAL_MACHINE),
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Hive::Hkcu => "HKCU",
            Hive::Hklm => "HKLM",
        }
    }
    /// Schrijven in HKLM vereist administratorrechten.
    pub fn needs_admin(self) -> bool {
        matches!(self, Hive::Hklm)
    }
}

/// Registerwaarde, inclusief "bestaat niet".
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "t", content = "v")]
pub enum RegData {
    Dword(u32),
    Qword(u64),
    Str(String),
    ExpandStr(String),
    MultiStr(Vec<String>),
    Binary(Vec<u8>),
    Absent,
}

impl RegData {
    /// Leesbare weergave voor het logboek.
    pub fn display(&self) -> String {
        match self {
            RegData::Dword(v) => format!("{v} (0x{v:x})"),
            RegData::Qword(v) => v.to_string(),
            RegData::Str(s) | RegData::ExpandStr(s) => format!("\"{s}\""),
            RegData::MultiStr(v) => v.join(" | "),
            RegData::Binary(b) => b.iter().map(|x| format!("{x:02x}")).collect::<Vec<_>>().join(" "),
            RegData::Absent => "(niet aanwezig)".to_string(),
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            RegData::Str(s) | RegData::ExpandStr(s) => Some(s.as_str()),
            _ => None,
        }
    }
}

fn utf16_to_string(bytes: &[u8]) -> String {
    let u16s: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    let end = u16s.iter().position(|&c| c == 0).unwrap_or(u16s.len());
    String::from_utf16_lossy(&u16s[..end])
}

fn string_to_utf16(s: &str) -> Vec<u8> {
    let mut out: Vec<u8> = s.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    out.extend_from_slice(&[0, 0]);
    out
}

fn from_reg_value(rv: &RegValue) -> RegData {
    match rv.vtype {
        REG_DWORD => {
            if rv.bytes.len() >= 4 {
                RegData::Dword(u32::from_le_bytes([rv.bytes[0], rv.bytes[1], rv.bytes[2], rv.bytes[3]]))
            } else {
                RegData::Binary(rv.bytes.to_vec())
            }
        }
        REG_QWORD => {
            if rv.bytes.len() >= 8 {
                let mut a = [0u8; 8];
                a.copy_from_slice(&rv.bytes[..8]);
                RegData::Qword(u64::from_le_bytes(a))
            } else {
                RegData::Binary(rv.bytes.to_vec())
            }
        }
        REG_SZ => RegData::Str(utf16_to_string(&rv.bytes)),
        REG_EXPAND_SZ => RegData::ExpandStr(utf16_to_string(&rv.bytes)),
        REG_MULTI_SZ => {
            let u16s: Vec<u16> = rv
                .bytes
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            let items: Vec<String> = u16s
                .split(|&c| c == 0)
                .filter(|s| !s.is_empty())
                .map(String::from_utf16_lossy)
                .collect();
            RegData::MultiStr(items)
        }
        _ => RegData::Binary(rv.bytes.to_vec()),
    }
}

fn to_reg_value(data: &RegData) -> Option<RegValue<'static>> {
    Some(match data {
        RegData::Dword(v) => RegValue { bytes: v.to_le_bytes().to_vec().into(), vtype: REG_DWORD },
        RegData::Qword(v) => RegValue { bytes: v.to_le_bytes().to_vec().into(), vtype: REG_QWORD },
        RegData::Str(s) => RegValue { bytes: string_to_utf16(s).into(), vtype: REG_SZ },
        RegData::ExpandStr(s) => RegValue { bytes: string_to_utf16(s).into(), vtype: REG_EXPAND_SZ },
        RegData::MultiStr(items) => {
            let mut bytes: Vec<u8> = Vec::new();
            for it in items {
                bytes.extend(string_to_utf16(it));
            }
            bytes.extend_from_slice(&[0, 0]);
            RegValue { bytes: bytes.into(), vtype: REG_MULTI_SZ }
        }
        RegData::Binary(b) => RegValue { bytes: b.clone().into(), vtype: REG_BINARY },
        RegData::Absent => return None,
    })
}

/// Leest een waarde. Ontbrekende sleutel of waarde geeft `RegData::Absent`.
pub fn read(hive: Hive, path: &str, name: &str) -> AppResult<RegData> {
    let key = match hive.root().open_subkey_with_flags(path, KEY_READ) {
        Ok(k) => k,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(RegData::Absent),
        Err(e) => return Err(map_access_denied(e, &format!("lezen van {}\\{}", hive.label(), path))),
    };
    match key.get_raw_value(name) {
        Ok(rv) => Ok(from_reg_value(&rv)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(RegData::Absent),
        Err(e) => Err(map_access_denied(e, &format!("lezen van {}\\{}\\{}", hive.label(), path, name))),
    }
}

/// Schrijft een waarde (maakt de sleutel zo nodig aan). `Absent` verwijdert de waarde.
pub fn write(hive: Hive, path: &str, name: &str, data: &RegData) -> AppResult<()> {
    let what = format!("schrijven naar {}\\{}\\{}", hive.label(), path, name);
    match data {
        RegData::Absent => {
            let key = match hive.root().open_subkey_with_flags(path, KEY_SET_VALUE) {
                Ok(k) => k,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
                Err(e) => return Err(map_access_denied(e, &what)),
            };
            match key.delete_value(name) {
                Ok(()) => Ok(()),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(e) => Err(map_access_denied(e, &what)),
            }
        }
        other => {
            let (key, _) = hive
                .root()
                .create_subkey_with_flags(path, KEY_SET_VALUE | KEY_READ)
                .map_err(|e| map_access_denied(e, &what))?;
            let rv = to_reg_value(other).ok_or_else(|| AppError::msg("ongeldige registerwaarde"))?;
            key.set_raw_value(name, &rv).map_err(|e| map_access_denied(e, &what))
        }
    }
}

/// Namen van subsleutels (lege lijst als de sleutel niet bestaat).
pub fn subkeys(hive: Hive, path: &str) -> AppResult<Vec<String>> {
    match hive.root().open_subkey_with_flags(path, KEY_READ) {
        Ok(k) => Ok(k.enum_keys().filter_map(|r| r.ok()).collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(map_access_denied(e, &format!("lezen van {}\\{}", hive.label(), path))),
    }
}

/// Alle waarden (naam, data) onder een sleutel.
pub fn values(hive: Hive, path: &str) -> AppResult<Vec<(String, RegData)>> {
    match hive.root().open_subkey_with_flags(path, KEY_READ) {
        Ok(k) => Ok(k
            .enum_values()
            .filter_map(|r| r.ok())
            .map(|(n, rv)| (n, from_reg_value(&rv)))
            .collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(vec![]),
        Err(e) => Err(map_access_denied(e, &format!("lezen van {}\\{}", hive.label(), path))),
    }
}

pub fn full_path(hive: Hive, path: &str, name: &str) -> String {
    format!("{}\\{}\\{}", hive.label(), path, name)
}
