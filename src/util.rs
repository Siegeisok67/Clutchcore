use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Local, TimeZone};

pub fn data_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("CLUTCHCORE_DATA") {
        if !custom.trim().is_empty() {
            return PathBuf::from(custom);
        }
    }
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."));
    base.join("Clutchcore")
}

pub fn snapshots_dir() -> PathBuf {
    data_dir().join("snapshots")
}

pub fn ensure_dirs() -> std::io::Result<()> {
    std::fs::create_dir_all(data_dir())?;
    std::fs::create_dir_all(snapshots_dir())?;
    Ok(())
}

pub fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

pub fn fmt_datetime(unix: u64) -> String {
    match Local.timestamp_opt(unix as i64, 0).single() {
        Some(dt) => dt.format("%b %d, %Y · %H:%M").to_string(),
        None => "unknown time".into(),
    }
}

pub fn fmt_time(unix: u64) -> String {
    match Local.timestamp_opt(unix as i64, 0).single() {
        Some(dt) => dt.format("%H:%M:%S").to_string(),
        None => "--:--:--".into(),
    }
}

pub fn fmt_relative(unix: u64) -> String {
    let now = unix_now();
    if unix == 0 || unix > now {
        return "just now".into();
    }
    let secs = now - unix;
    if secs < 60 {
        "just now".into()
    } else if secs < 3600 {
        format!("{} min ago", secs / 60)
    } else if secs < 86400 {
        format!("{} hr ago", secs / 3600)
    } else {
        format!("{} days ago", secs / 86400)
    }
}

pub fn file_stamp(unix: u64) -> String {
    match Local.timestamp_opt(unix as i64, 0).single() {
        Some(dt) => dt.format("%Y%m%d-%H%M%S").to_string(),
        None => format!("{unix}"),
    }
}

pub fn hex_encode(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

pub fn hex_decode(s: &str) -> Option<Vec<u8>> {
    let cleaned: Vec<char> = s.chars().filter(|c| !c.is_whitespace()).collect();
    if cleaned.len() % 2 != 0 {
        return None;
    }
    let mut out = Vec::with_capacity(cleaned.len() / 2);
    let mut i = 0;
    while i < cleaned.len() {
        let hi = cleaned[i].to_digit(16)?;
        let lo = cleaned[i + 1].to_digit(16)?;
        out.push(((hi << 4) | lo) as u8);
        i += 2;
    }
    Some(out)
}

pub fn truncate(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

pub fn human_gb(gb: f64) -> String {
    if gb >= 1000.0 {
        format!("{:.1} TB", gb / 1024.0)
    } else if gb >= 10.0 {
        format!("{gb:.0} GB")
    } else {
        format!("{gb:.1} GB")
    }
}

pub fn human_uptime(secs: u64) -> String {
    let days = secs / 86400;
    let hours = (secs % 86400) / 3600;
    let mins = (secs % 3600) / 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else {
        format!("{mins}m")
    }
}

pub fn open_path(path: &Path) {
    let _ = Command::new("explorer").arg(path).spawn();
}

pub fn open_uri(uri: &str) {
    let _ = Command::new("cmd")
        .args(["/C", "start", "", uri])
        .spawn();
}

pub fn open_url(url: &str) {
    open_uri(url);
}

pub fn expand_env(input: &str) -> String {
    let mut out = String::new();
    let bytes: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == '%' {
            if let Some(end) = bytes[i + 1..].iter().position(|c| *c == '%') {
                let name: String = bytes[i + 1..i + 1 + end].iter().collect();
                if let Ok(val) = std::env::var(&name) {
                    out.push_str(&val);
                    i = i + end + 2;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    out
}

pub fn write_text(path: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, contents)
}
