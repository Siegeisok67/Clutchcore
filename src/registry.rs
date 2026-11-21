use serde::{Deserialize, Serialize};
use winreg::enums::{RegType, *};
use winreg::{RegKey, RegValue};

use crate::model::{DesiredValue, Hive};
use crate::util::{hex_decode, hex_encode};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RawValue {
    pub vtype: String,
    pub data: String,
}

impl RawValue {
    pub fn dword(v: u32) -> Self {
        Self {
            vtype: "REG_DWORD".into(),
            data: hex_encode(&v.to_le_bytes()),
        }
    }

    pub fn string(v: &str) -> Self {
        let mut bytes = Vec::new();
        for unit in v.encode_utf16() {
            bytes.extend_from_slice(&unit.to_le_bytes());
        }
        bytes.extend_from_slice(&[0, 0]);
        Self {
            vtype: "REG_SZ".into(),
            data: hex_encode(&bytes),
        }
    }

    pub fn bytes(&self) -> Option<Vec<u8>> {
        hex_decode(&self.data)
    }

    pub fn to_u32(&self) -> Option<u32> {
        if self.vtype != "REG_DWORD" {
            return None;
        }
        let b = self.bytes()?;
        if b.len() < 4 {
            return None;
        }
        Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    pub fn to_u64(&self) -> Option<u64> {
        if self.vtype != "REG_QWORD" {
            return None;
        }
        let b = self.bytes()?;
        if b.len() < 8 {
            return None;
        }
        Some(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    pub fn to_string_value(&self) -> Option<String> {
        if self.vtype != "REG_SZ" && self.vtype != "REG_EXPAND_SZ" && self.vtype != "REG_MULTI_SZ" {
            return None;
        }
        let b = self.bytes()?;
        Some(decode_utf16(&b))
    }

    pub fn display(&self) -> String {
        match self.vtype.as_str() {
            "REG_DWORD" => self
                .to_u32()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "<invalid dword>".into()),
            "REG_QWORD" => self
                .to_u64()
                .map(|v| v.to_string())
                .unwrap_or_else(|| "<invalid qword>".into()),
            "REG_SZ" | "REG_EXPAND_SZ" => self
                .to_string_value()
                .map(|s| format!("\"{s}\""))
                .unwrap_or_else(|| "<invalid string>".into()),
            "REG_MULTI_SZ" => self
                .to_string_value()
                .map(|s| format!("\"{}\"", s.replace('\0', " · ")))
                .unwrap_or_else(|| "<invalid multi-string>".into()),
            "REG_BINARY" => {
                if let Some(b) = self.bytes() {
                    let mut s = String::from("0x");
                    for byte in b.iter().take(16) {
                        s.push_str(&format!("{byte:02X}"));
                    }
                    if b.len() > 16 {
                        s.push('…');
                    }
                    s
                } else {
                    "<invalid binary>".into()
                }
            }
            other => format!("<{other}>"),
        }
    }

    pub fn type_label(&self) -> &str {
        &self.vtype
    }

    pub fn to_reg_value(&self) -> Result<RegValue, String> {
        let bytes = self
            .bytes()
            .ok_or_else(|| "stored registry bytes are corrupt".to_string())?;
        let vtype = type_from_name(&self.vtype)
            .ok_or_else(|| format!("unsupported registry type {}", self.vtype))?;
        Ok(RegValue { bytes, vtype })
    }

    pub fn from_reg_value(v: RegValue) -> Self {
        Self {
            vtype: type_name(v.vtype).to_string(),
            data: hex_encode(&v.bytes),
        }
    }
}

impl DesiredValue {
    pub fn to_raw(self) -> RawValue {
        match self {
            DesiredValue::Dword(v) => RawValue::dword(v),
            DesiredValue::Str(s) => RawValue::string(s),
        }
    }

    pub fn display(self) -> String {
        match self {
            DesiredValue::Dword(v) => v.to_string(),
            DesiredValue::Str(s) => format!("\"{s}\""),
        }
    }
}

fn type_name(vtype: RegType) -> &'static str {
    match vtype {
        RegType::REG_SZ => "REG_SZ",
        RegType::REG_EXPAND_SZ => "REG_EXPAND_SZ",
        RegType::REG_MULTI_SZ => "REG_MULTI_SZ",
        RegType::REG_DWORD => "REG_DWORD",
        RegType::REG_QWORD => "REG_QWORD",
        RegType::REG_BINARY => "REG_BINARY",
        RegType::REG_NONE => "REG_NONE",
        _ => "REG_UNKNOWN",
    }
}

fn type_from_name(name: &str) -> Option<RegType> {
    match name {
        "REG_SZ" => Some(RegType::REG_SZ),
        "REG_EXPAND_SZ" => Some(RegType::REG_EXPAND_SZ),
        "REG_MULTI_SZ" => Some(RegType::REG_MULTI_SZ),
        "REG_DWORD" => Some(RegType::REG_DWORD),
        "REG_QWORD" => Some(RegType::REG_QWORD),
        "REG_BINARY" => Some(RegType::REG_BINARY),
        "REG_NONE" => Some(RegType::REG_NONE),
        _ => None,
    }
}

fn decode_utf16(bytes: &[u8]) -> String {
    let mut units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]))
        .collect();
    while units.last() == Some(&0) {
        units.pop();
    }
    String::from_utf16_lossy(&units)
}

pub fn predef(hive: Hive) -> RegKey {
    match hive {
        Hive::Hkcu => RegKey::predef(HKEY_CURRENT_USER),
        Hive::Hklm => RegKey::predef(HKEY_LOCAL_MACHINE),
    }
}

pub fn read(hive: Hive, path: &str, name: &str) -> Result<Option<RawValue>, String> {
    match predef(hive).open_subkey_with_flags(path, KEY_READ) {
        Ok(key) => match key.get_raw_value(name) {
            Ok(v) => Ok(Some(RawValue::from_reg_value(v))),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(format!("{e}")),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("{e}")),
    }
}

pub fn value_exists(hive: Hive, path: &str, name: &str) -> bool {
    matches!(read(hive, path, name), Ok(Some(_)))
}

pub fn write_raw(hive: Hive, path: &str, name: &str, value: &RawValue) -> Result<(), String> {
    let root = predef(hive);
    let (key, _) = root
        .create_subkey(path)
        .map_err(|e| format!("cannot open/create {path}: {e}"))?;
    let rv = value.to_reg_value()?;
    key.set_raw_value(name, &rv)
        .map_err(|e| format!("cannot write {name}: {e}"))
}

pub fn write_desired(hive: Hive, path: &str, name: &str, desired: DesiredValue) -> Result<(), String> {
    write_raw(hive, path, name, &desired.to_raw())
}

pub fn delete_value(hive: Hive, path: &str, name: &str) -> Result<(), String> {
    match predef(hive).open_subkey_with_flags(path, KEY_SET_VALUE | KEY_READ) {
        Ok(key) => match key.delete_value(name) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(format!("cannot remove {name}: {e}")),
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("cannot open {path} for cleanup: {e}")),
    }
}

pub fn list_subkeys(hive: Hive, path: &str) -> Vec<String> {
    match predef(hive).open_subkey_with_flags(path, KEY_READ) {
        Ok(key) => key.enum_keys().filter_map(|k| k.ok()).collect(),
        Err(_) => Vec::new(),
    }
}

pub fn list_values(hive: Hive, path: &str) -> Vec<(String, RawValue)> {
    match predef(hive).open_subkey_with_flags(path, KEY_READ) {
        Ok(key) => key
            .enum_values()
            .filter_map(|v| v.ok())
            .map(|(name, value)| (name, RawValue::from_reg_value(value)))
            .collect(),
        Err(_) => Vec::new(),
    }
}

pub fn read_u32(hive: Hive, path: &str, name: &str) -> Option<u32> {
    read(hive, path, name).ok().flatten().and_then(|v| v.to_u32())
}

pub fn read_string(hive: Hive, path: &str, name: &str) -> Option<String> {
    read(hive, path, name)
        .ok()
        .flatten()
        .and_then(|v| v.to_string_value())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RegBackup {
    pub hive: String,
    pub path: String,
    pub name: String,
    pub existed: bool,
    pub value: Option<RawValue>,
}

impl RegBackup {
    pub fn capture(hive: Hive, path: &str, name: &str) -> Result<Self, String> {
        let value = read(hive, path, name)?;
        Ok(Self {
            hive: hive.label().to_string(),
            path: path.to_string(),
            name: name.to_string(),
            existed: value.is_some(),
            value,
        })
    }

    pub fn hive(&self) -> Result<Hive, String> {
        match self.hive.as_str() {
            "HKCU" => Ok(Hive::Hkcu),
            "HKLM" => Ok(Hive::Hklm),
            other => Err(format!("unknown hive {other}")),
        }
    }

    pub fn restore(&self) -> Result<(), String> {
        let hive = self.hive()?;
        if !self.existed {
            delete_value(hive, &self.path, &self.name)
        } else if let Some(value) = &self.value {
            write_raw(hive, &self.path, &self.name, value)
        } else {
            Err("backup marked as existing but has no value".into())
        }
    }

    pub fn current_value(&self) -> Option<RawValue> {
        self.hive().ok().and_then(|h| read(h, &self.path, &self.name).ok().flatten())
    }

    pub fn path_label(&self) -> String {
        if self.name.is_empty() {
            format!("{}\\{}", self.hive, self.path)
        } else {
            format!("{}\\{} · {}", self.hive, self.path, self.name)
        }
    }
}

pub fn matches_desired(current: &Option<RawValue>, desired: DesiredValue) -> bool {
    let Some(current) = current else {
        return false;
    };
    match desired {
        DesiredValue::Dword(v) => current.to_u32() == Some(v),
        DesiredValue::Str(s) => current.to_string_value().as_deref() == Some(s),
    }
}
