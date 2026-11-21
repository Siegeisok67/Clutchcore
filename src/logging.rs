use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

use chrono::Local;
use serde::Serialize;

use crate::util;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub enum Level {
    Info,
    Success,
    Warn,
    Error,
}

impl Level {
    pub fn tag(self) -> &'static str {
        match self {
            Level::Info => "INFO",
            Level::Success => "OK",
            Level::Warn => "WARN",
            Level::Error => "ERROR",
        }
    }
}

#[derive(Clone, Debug)]
pub struct LogEntry {
    pub time: String,
    pub level: Level,
    pub message: String,
}

struct Logger {
    entries: Vec<LogEntry>,
    file: PathBuf,
}

static LOGGER: OnceLock<Mutex<Logger>> = OnceLock::new();

pub fn init() {
    let _ = util::ensure_dirs();
    let path = util::data_dir().join("clutchcore.log");
    LOGGER.get_or_init(|| {
        Mutex::new(Logger {
            entries: Vec::new(),
            file: path,
        })
    });
}

fn with_logger(f: impl FnOnce(&mut Logger)) {
    if let Some(lock) = LOGGER.get() {
        if let Ok(mut logger) = lock.lock() {
            f(&mut logger);
        }
    }
}

pub fn log(level: Level, message: impl AsRef<str>) {
    let message = message.as_ref().to_string();
    let time = Local::now().format("%H:%M:%S").to_string();
    let line = format!(
        "{} [{}] {}\n",
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        level.tag(),
        message
    );
    with_logger(|logger| {
        logger.entries.push(LogEntry {
            time,
            level,
            message,
        });
        if logger.entries.len() > 600 {
            let drain = logger.entries.len() - 600;
            logger.entries.drain(0..drain);
        }
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&logger.file)
        {
            let _ = file.write_all(line.as_bytes());
        }
    });
    if cfg!(debug_assertions) {
        eprintln!("[{}] {}", level.tag(), line.trim_end());
    }
}

pub fn info(message: impl AsRef<str>) {
    log(Level::Info, message);
}

pub fn success(message: impl AsRef<str>) {
    log(Level::Success, message);
}

pub fn warn(message: impl AsRef<str>) {
    log(Level::Warn, message);
}

pub fn error(message: impl AsRef<str>) {
    log(Level::Error, message);
}

pub fn entries() -> Vec<LogEntry> {
    let mut out = Vec::new();
    with_logger(|logger| out = logger.entries.clone());
    out
}

pub fn log_file() -> PathBuf {
    util::data_dir().join("clutchcore.log")
}
