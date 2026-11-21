use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::presets;
use crate::util;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tweak_ids: Vec<String>,
    pub custom: bool,
    pub game: Option<String>,
    pub launch_options: String,
    pub config_path: String,
    pub in_game: Vec<String>,
    pub notes: Vec<String>,
    pub provider: Option<String>,
    pub created_unix: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ProfileStore {
    #[serde(default)]
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub active: Option<String>,
}

impl ProfileStore {
    pub fn custom(&self) -> Vec<&Profile> {
        self.profiles.iter().filter(|p| p.custom).collect()
    }

    pub fn find_custom(&self, id: &str) -> Option<&Profile> {
        self.profiles.iter().find(|p| p.id == id)
    }

    pub fn find_custom_mut(&mut self, id: &str) -> Option<&mut Profile> {
        self.profiles.iter_mut().find(|p| p.id == id)
    }
}

pub fn from_game(p: &presets::GamePreset) -> Profile {
    Profile {
        id: format!("preset-{}", p.id),
        name: p.game.to_string(),
        description: p.tagline.to_string(),
        tweak_ids: p.tweak_ids.iter().map(|s| s.to_string()).collect(),
        custom: false,
        game: Some(p.game.to_string()),
        launch_options: p.launch_options.to_string(),
        config_path: p.config_path.to_string(),
        in_game: p.in_game.iter().map(|s| s.to_string()).collect(),
        notes: p.notes.iter().map(|s| s.to_string()).collect(),
        provider: Some(p.provider.to_string()),
        created_unix: 0,
    }
}

pub fn from_system(p: &presets::SystemPreset) -> Profile {
    Profile {
        id: format!("preset-{}", p.id),
        name: p.name.to_string(),
        description: p.description.to_string(),
        tweak_ids: p.tweak_ids.iter().map(|s| s.to_string()).collect(),
        custom: false,
        game: None,
        launch_options: String::new(),
        config_path: String::new(),
        in_game: Vec::new(),
        notes: vec![p.tagline.to_string()],
        provider: None,
        created_unix: 0,
    }
}

pub fn builtin() -> Vec<Profile> {
    let mut out: Vec<Profile> = presets::all().iter().map(from_game).collect();
    out.extend(presets::system_all().iter().map(from_system));
    out
}

pub fn all(store: &ProfileStore) -> Vec<Profile> {
    let mut out = builtin();
    out.extend(store.profiles.iter().cloned());
    out
}

pub fn find(store: &ProfileStore, id: &str) -> Option<Profile> {
    all(store).into_iter().find(|p| p.id == id)
}

pub fn active_name(store: &ProfileStore) -> Option<String> {
    store
        .active
        .as_ref()
        .and_then(|id| find(store, id))
        .map(|p| p.name)
}

pub fn store_path() -> PathBuf {
    util::data_dir().join("profiles.json")
}

pub fn load_store() -> ProfileStore {
    let mut store: ProfileStore = std::fs::read_to_string(store_path())
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    store.profiles.retain(|p| p.custom);
    store
}

pub fn save_store(store: &ProfileStore) -> Result<(), String> {
    util::ensure_dirs().map_err(|e| format!("could not create the data folder: {e}"))?;
    let json = serde_json::to_string_pretty(store)
        .map_err(|e| format!("could not serialize profiles: {e}"))?;
    util::write_text(&store_path(), &json)
        .map_err(|e| format!("could not write profiles.json: {e}"))
}

pub fn upsert_custom(
    store: &mut ProfileStore,
    id: Option<&str>,
    name: &str,
    description: &str,
    tweak_ids: Vec<String>,
) -> Result<Profile, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Give the profile a name.".into());
    }
    if tweak_ids.is_empty() {
        return Err("Select at least one tweak for the profile.".into());
    }
    let description = description.trim().to_string();
    if let Some(id) = id {
        if let Some(existing) = store.find_custom_mut(id) {
            existing.name = name.to_string();
            existing.description = description;
            existing.tweak_ids = tweak_ids;
            let updated = existing.clone();
            save_store(store)?;
            return Ok(updated);
        }
    }
    let now = util::unix_now();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0);
    let profile = Profile {
        id: format!("custom-{now}-{nanos}"),
        name: name.to_string(),
        description,
        tweak_ids,
        custom: true,
        game: None,
        launch_options: String::new(),
        config_path: String::new(),
        in_game: Vec::new(),
        notes: Vec::new(),
        provider: None,
        created_unix: now,
    };
    store.profiles.push(profile.clone());
    save_store(store)?;
    Ok(profile)
}

pub fn delete_custom(store: &mut ProfileStore, id: &str) -> Result<(), String> {
    let before = store.profiles.len();
    store.profiles.retain(|p| p.id != id);
    if store.profiles.len() == before {
        return Err("Profile not found.".into());
    }
    if store.active.as_deref() == Some(id) {
        store.active = None;
    }
    save_store(store)
}

pub fn set_active(store: &mut ProfileStore, id: Option<&str>) -> Result<(), String> {
    store.active = id.map(|s| s.to_string());
    save_store(store)
}

pub fn profile_tweak_ids(profile: &Profile) -> Vec<String> {
    profile
        .tweak_ids
        .iter()
        .filter(|id| crate::tweaks::find(id).is_some())
        .cloned()
        .collect()
}
