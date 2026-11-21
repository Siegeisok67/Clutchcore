use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::registry::{RegBackup, RawValue};
use crate::util;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SnapshotEntry {
    pub tweak_id: String,
    pub tweak_name: String,
    pub captured_unix: u64,
    #[serde(default)]
    pub regs: Vec<RegBackup>,
    #[serde(default)]
    pub power_plan: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SavedResult {
    pub name: String,
    pub ok: bool,
    pub skipped: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub id: String,
    pub created_unix: u64,
    pub label: String,
    #[serde(default)]
    pub profile: Option<String>,
    #[serde(default)]
    pub entries: Vec<SnapshotEntry>,
    #[serde(default)]
    pub results: Vec<SavedResult>,
    #[serde(default)]
    pub restored: bool,
    #[serde(default)]
    pub app_version: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Originals {
    #[serde(default)]
    pub created_unix: u64,
    #[serde(default)]
    pub updated_unix: u64,
    #[serde(default)]
    pub entries: BTreeMap<String, SnapshotEntry>,
}

pub fn snapshots_dir() -> PathBuf {
    util::snapshots_dir()
}

pub fn new_id() -> String {
    let now = util::unix_now();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    format!("snap-{now}-{nanos}")
}

fn safe_id(id: &str) -> String {
    id.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

pub fn save_snapshot(snapshot: &Snapshot) -> Result<PathBuf, String> {
    util::ensure_dirs().map_err(|e| format!("could not create the data folder: {e}"))?;
    let path = snapshots_dir().join(format!("{}.json", safe_id(&snapshot.id)));
    let json = serde_json::to_string_pretty(snapshot)
        .map_err(|e| format!("could not serialize the snapshot: {e}"))?;
    util::write_text(&path, &json).map_err(|e| format!("could not write the snapshot: {e}"))?;
    Ok(path)
}

pub fn list_snapshots() -> Vec<Snapshot> {
    let dir = snapshots_dir();
    let mut out = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(snapshot) = serde_json::from_str::<Snapshot>(&text) {
                    out.push(snapshot);
                }
            }
        }
    }
    out.sort_by(|a, b| b.created_unix.cmp(&a.created_unix));
    out
}

pub fn load_snapshot(id: &str) -> Option<Snapshot> {
    let path = snapshots_dir().join(format!("{}.json", safe_id(id)));
    std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str::<Snapshot>(&text).ok())
        .or_else(|| list_snapshots().into_iter().find(|s| s.id == id))
}

pub fn originals_path() -> PathBuf {
    util::data_dir().join("originals.json")
}

pub fn load_originals() -> Originals {
    std::fs::read_to_string(originals_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

pub fn save_originals(originals: &Originals) -> Result<(), String> {
    util::ensure_dirs().map_err(|e| format!("could not create the data folder: {e}"))?;
    let json = serde_json::to_string_pretty(originals)
        .map_err(|e| format!("could not serialize original settings: {e}"))?;
    util::write_text(&originals_path(), &json)
        .map_err(|e| format!("could not write originals.json: {e}"))
}

pub fn record_originals(originals: &mut Originals, entry: SnapshotEntry) {
    let now = util::unix_now();
    if originals.created_unix == 0 {
        originals.created_unix = now;
    }
    originals.updated_unix = now;
    originals.entries.entry(entry.tweak_id.clone()).or_insert(entry);
}

pub fn value_or_missing(value: Option<&RawValue>) -> String {
    match value {
        Some(v) => format!("{} ({})", v.display(), v.type_label()),
        None => "<not set>".into(),
    }
}

pub fn entry_text(entry: &SnapshotEntry) -> String {
    let mut out = String::new();
    out.push_str(&format!("{}\n", entry.tweak_name));
    out.push_str(&format!(
        "  captured {}\n",
        util::fmt_datetime(entry.captured_unix)
    ));
    for reg in &entry.regs {
        let original = value_or_missing(reg.value.as_ref());
        let current = reg.current_value();
        let current_text = value_or_missing(current.as_ref());
        out.push_str(&format!("  {}\n", reg.path_label()));
        out.push_str(&format!("    original: {original}\n"));
        out.push_str(&format!("    now:      {current_text}\n"));
    }
    if let Some(plan) = &entry.power_plan {
        out.push_str(&format!("  power plan GUID: {plan}\n"));
    }
    out.push('\n');
    out
}

pub fn snapshot_text(snapshot: &Snapshot) -> String {
    let mut out = String::new();
    out.push_str("CLUTCHCORE — SETTINGS SNAPSHOT\n");
    out.push_str("========================================\n");
    out.push_str(&format!("Snapshot: {}\n", snapshot.label));
    out.push_str(&format!("Created:  {}\n", util::fmt_datetime(snapshot.created_unix)));
    out.push_str(&format!("ID:       {}\n", snapshot.id));
    if let Some(profile) = &snapshot.profile {
        out.push_str(&format!("Profile:  {profile}\n"));
    }
    out.push_str(&format!("Tweaks:   {}\n\n", snapshot.entries.len()));
    for entry in &snapshot.entries {
        out.push_str(&entry_text(entry));
    }
    out.push_str("This file records the values that were active BEFORE Clutchcore changed them.\n");
    out
}

pub fn originals_text(originals: &Originals) -> String {
    let mut out = String::new();
    out.push_str("CLUTCHCORE — ORIGINAL SETTINGS VAULT\n");
    out.push_str("========================================\n");
    out.push_str("The first values Clutchcore ever saw for each tweak, captured before any change.\n");
    out.push_str(&format!(
        "Created: {}\n",
        if originals.created_unix == 0 {
            "not yet populated".to_string()
        } else {
            util::fmt_datetime(originals.created_unix)
        }
    ));
    out.push_str(&format!(
        "Updated: {}\n",
        if originals.updated_unix == 0 {
            "never".to_string()
        } else {
            util::fmt_datetime(originals.updated_unix)
        }
    ));
    out.push_str(&format!("Tweaks captured: {}\n\n", originals.entries.len()));
    for entry in originals.entries.values() {
        out.push_str(&entry_text(entry));
    }
    out
}

pub fn export_snapshot_txt(snapshot: &Snapshot) -> Result<PathBuf, String> {
    util::ensure_dirs().map_err(|e| format!("could not create data folder: {e}"))?;
    let path = util::data_dir().join(format!(
        "snapshot-{}-{}.txt",
        safe_id(&snapshot.id),
        util::file_stamp(util::unix_now())
    ));
    util::write_text(&path, &snapshot_text(snapshot))
        .map_err(|e| format!("could not write snapshot text: {e}"))?;
    Ok(path)
}

pub fn export_originals_txt(originals: &Originals) -> Result<PathBuf, String> {
    util::ensure_dirs().map_err(|e| format!("could not create data folder: {e}"))?;
    let path = util::data_dir().join("ORIGINAL-SETTINGS.txt");
    util::write_text(&path, &originals_text(originals))
        .map_err(|e| format!("could not write original settings text: {e}"))?;
    Ok(path)
}

pub fn latest_entry_for(originals: &Originals, snapshots: &[Snapshot], tweak_id: &str) -> Option<SnapshotEntry> {
    originals.entries.get(tweak_id).cloned().or_else(|| {
        snapshots
            .iter()
            .find_map(|s| s.entries.iter().find(|e| e.tweak_id == tweak_id).cloned())
    })
}
