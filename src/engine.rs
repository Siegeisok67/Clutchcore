use crate::backup::{self, SavedResult, Snapshot, SnapshotEntry};
use crate::logging;
use crate::model::*;
use crate::registry::{self, RegBackup};
use crate::system;
use crate::tweaks::{self, ResolvedTarget};
use crate::util;

#[derive(Clone, Debug)]
pub struct TweakResult {
    pub tweak_id: String,
    pub name: String,
    pub ok: bool,
    pub skipped: bool,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Default)]
pub struct ApplyReport {
    pub label: String,
    pub results: Vec<TweakResult>,
    pub snapshot_id: Option<String>,
    pub needs_reboot: bool,
    pub needs_admin: bool,
    pub fatal: Option<String>,
}

impl ApplyReport {
    pub fn applied(&self) -> usize {
        self.results.iter().filter(|r| r.ok && !r.skipped).count()
    }

    pub fn failed(&self) -> usize {
        self.results.iter().filter(|r| !r.ok).count()
    }

    pub fn skipped(&self) -> usize {
        self.results.iter().filter(|r| r.skipped).count()
    }

    pub fn summary(&self) -> String {
        if let Some(fatal) = &self.fatal {
            return fatal.clone();
        }
        if self.failed() > 0 {
            let first = self
                .results
                .iter()
                .find(|r| !r.ok && r.error.is_some())
                .and_then(|r| r.error.clone())
                .unwrap_or_else(|| "unknown error".into());
            return format!(
                "{} applied · {} skipped · {} failed ({})",
                self.applied(),
                self.skipped(),
                self.failed(),
                first
            );
        }
        format!(
            "{} applied · {} already active",
            self.applied(),
            self.skipped()
        )
    }
}

fn spec_ids(ids: &[String]) -> Vec<&'static TweakSpec> {
    let mut out: Vec<&'static TweakSpec> = Vec::new();
    for id in ids {
        if let Some(spec) = tweaks::find(id) {
            if !out.iter().any(|s| s.id == spec.id) {
                out.push(spec);
            }
        }
    }
    out
}

pub fn apply(ids: &[String], label: &str, profile: Option<&str>) -> ApplyReport {
    let mut report = ApplyReport {
        label: label.to_string(),
        ..Default::default()
    };
    let specs = spec_ids(ids);
    if specs.is_empty() {
        report.fatal = Some("No valid tweaks were selected.".into());
        return report;
    }

    let plan = system::power_plan();
    let mut work: Vec<(&'static TweakSpec, Vec<ResolvedTarget>)> = Vec::new();

    for spec in &specs {
        if matches!(spec.kind, TweakKind::PowerPlan) && plan.is_none() {
            report.results.push(TweakResult {
                tweak_id: spec.id.to_string(),
                name: spec.name.to_string(),
                ok: false,
                skipped: false,
                error: Some(
                    "Could not read the current power plan, so Clutchcore refused to change it."
                        .into(),
                ),
            });
            continue;
        }
        let status = tweaks::detect(spec, plan.as_ref());
        if status.state == TweakState::Applied {
            report.results.push(TweakResult {
                tweak_id: spec.id.to_string(),
                name: spec.name.to_string(),
                ok: true,
                skipped: true,
                error: None,
            });
            continue;
        }
        match tweaks::resolve_targets(spec) {
            Ok(targets) => work.push((spec, targets)),
            Err(error) => report.results.push(TweakResult {
                tweak_id: spec.id.to_string(),
                name: spec.name.to_string(),
                ok: false,
                skipped: false,
                error: Some(error),
            }),
        }
    }

    if work.is_empty() {
        logging::info(format!("{label}: nothing to change"));
        return report;
    }

    let now = util::unix_now();
    let mut entries: Vec<SnapshotEntry> = Vec::new();
    let mut capture_error: Option<String> = None;

    for (spec, targets) in &work {
        let mut entry = SnapshotEntry {
            tweak_id: spec.id.to_string(),
            tweak_name: spec.name.to_string(),
            captured_unix: now,
            regs: Vec::new(),
            power_plan: None,
        };
        if matches!(spec.kind, TweakKind::PowerPlan) {
            entry.power_plan = plan.as_ref().map(|p| p.guid.clone());
        } else {
            for target in targets {
                match RegBackup::capture(target.hive, &target.path, &target.name) {
                    Ok(backup) => entry.regs.push(backup),
                    Err(error) => {
                        capture_error = Some(format!(
                            "Could not read {} before changing it: {error}",
                            spec.name
                        ));
                    }
                }
            }
        }
        entries.push(entry);
    }

    if let Some(error) = capture_error {
        report.fatal = Some(format!(
            "Nothing was changed. {error} Run Clutchcore as administrator and try again."
        ));
        logging::error(&error);
        return report;
    }

    let snapshot_id = backup::new_id();
    let mut snapshot = Snapshot {
        id: snapshot_id.clone(),
        created_unix: now,
        label: label.to_string(),
        profile: profile.map(|p| p.to_string()),
        entries: entries.clone(),
        results: Vec::new(),
        restored: false,
        app_version: env!("CARGO_PKG_VERSION").into(),
    };

    // The backup is written before any registry value is touched.
    if let Err(error) = backup::save_snapshot(&snapshot) {
        report.fatal = Some(format!("Backup failed, so nothing was changed: {error}"));
        logging::error(format!("Backup failed, aborting apply: {error}"));
        return report;
    }
    report.snapshot_id = Some(snapshot_id.clone());

    let mut originals = backup::load_originals();
    for entry in &entries {
        backup::record_originals(&mut originals, entry.clone());
    }
    if let Err(error) = backup::save_originals(&originals) {
        logging::warn(format!("Could not update originals.json: {error}"));
    }
    if let Err(error) = backup::export_originals_txt(&originals) {
        logging::warn(format!("Could not refresh the readable original settings: {error}"));
    }

    for ((spec, targets), entry) in work.iter().zip(entries.iter()) {
        let mut errors: Vec<String> = Vec::new();
        if let Some(guid) = &entry.power_plan {
            logging::info(format!(
                "Backed up active power plan {guid} before switching"
            ));
        }
        match spec.kind {
            TweakKind::PowerPlan => {
                if let Err(error) = tweaks::set_power_plan_high() {
                    errors.push(error);
                }
            }
            _ => {
                for target in targets {
                    if let Err(error) = registry::write_desired(
                        target.hive,
                        &target.path,
                        &target.name,
                        target.desired,
                    ) {
                        errors.push(error);
                    }
                }
            }
        }
        let ok = errors.is_empty();
        if ok {
            logging::success(format!("Applied: {}", spec.name));
        } else {
            logging::error(format!(
                "Failed: {} — {}",
                spec.name,
                errors.join("; ")
            ));
        }
        report.results.push(TweakResult {
            tweak_id: spec.id.to_string(),
            name: spec.name.to_string(),
            ok,
            skipped: false,
            error: if errors.is_empty() {
                None
            } else {
                Some(errors.join("; "))
            },
        });
        if spec.requires_reboot && ok {
            report.needs_reboot = true;
        }
    }

    report.needs_admin = report.failed() > 0 && !system::is_admin();

    snapshot.results = report
        .results
        .iter()
        .map(|r| SavedResult {
            name: r.name.clone(),
            ok: r.ok,
            skipped: r.skipped,
            error: r.error.clone(),
        })
        .collect();
    if let Err(error) = backup::save_snapshot(&snapshot) {
        logging::warn(format!("Could not attach results to the snapshot: {error}"));
    }

    report
}

fn restore_entry(entry: &SnapshotEntry) -> Vec<String> {
    let mut errors = Vec::new();
    for reg in entry.regs.iter().rev() {
        if let Err(error) = reg.restore() {
            errors.push(error);
        }
    }
    if let Some(guid) = &entry.power_plan {
        if let Err(error) = tweaks::restore_power_plan(guid) {
            errors.push(error);
        }
    }
    errors
}

fn capture_current(entry: &SnapshotEntry) -> SnapshotEntry {
    let mut current = SnapshotEntry {
        tweak_id: entry.tweak_id.clone(),
        tweak_name: entry.tweak_name.clone(),
        captured_unix: util::unix_now(),
        regs: Vec::new(),
        power_plan: None,
    };
    for reg in &entry.regs {
        if let Ok(hive) = reg.hive() {
            match registry::read(hive, &reg.path, &reg.name) {
                Ok(Some(value)) => current.regs.push(RegBackup {
                    hive: reg.hive.clone(),
                    path: reg.path.clone(),
                    name: reg.name.clone(),
                    existed: true,
                    value: Some(value),
                }),
                Ok(None) => current.regs.push(RegBackup {
                    hive: reg.hive.clone(),
                    path: reg.path.clone(),
                    name: reg.name.clone(),
                    existed: false,
                    value: None,
                }),
                Err(_) => {}
            }
        }
    }
    if entry.power_plan.is_some() {
        current.power_plan = system::power_plan().map(|p| p.guid);
    }
    current
}

fn make_pre_restore_snapshot(label: &str, entries: &[SnapshotEntry]) -> Option<String> {
    if entries.is_empty() {
        return None;
    }
    let id = backup::new_id();
    let snapshot = Snapshot {
        id: id.clone(),
        created_unix: util::unix_now(),
        label: label.to_string(),
        profile: None,
        entries: entries.to_vec(),
        results: Vec::new(),
        restored: false,
        app_version: env!("CARGO_PKG_VERSION").into(),
    };
    match backup::save_snapshot(&snapshot) {
        Ok(_) => Some(id),
        Err(error) => {
            logging::warn(format!("Could not save the pre-restore snapshot: {error}"));
            None
        }
    }
}

pub fn revert(ids: &[String], label: &str) -> ApplyReport {
    let mut report = ApplyReport {
        label: label.to_string(),
        ..Default::default()
    };
    let specs = spec_ids(ids);
    if specs.is_empty() {
        report.fatal = Some("No valid tweaks were selected.".into());
        return report;
    }

    let originals = backup::load_originals();
    let snapshots = backup::list_snapshots();
    let mut entries: Vec<SnapshotEntry> = Vec::new();

    for spec in &specs {
        if let Some(entry) = backup::latest_entry_for(&originals, &snapshots, spec.id) {
            entries.push(capture_current(&entry));
        } else {
            report.results.push(TweakResult {
                tweak_id: spec.id.to_string(),
                name: spec.name.to_string(),
                ok: false,
                skipped: false,
                error: Some("No original values have been captured for this tweak yet.".into()),
            });
        }
    }

    if entries.is_empty() {
        report.fatal = Some("No original values are stored for these tweaks yet.".into());
        return report;
    }

    report.snapshot_id = make_pre_restore_snapshot(&format!("Before revert: {label}"), &entries);

    let originals = backup::load_originals();
    let snapshots = backup::list_snapshots();
    for spec in &specs {
        let Some(entry) = backup::latest_entry_for(&originals, &snapshots, spec.id) else {
            continue;
        };
        let errors = restore_entry(&entry);
        let ok = errors.is_empty();
        if ok {
            logging::success(format!("Restored: {}", spec.name));
        } else {
            logging::error(format!("Restore failed: {} — {}", spec.name, errors.join("; ")));
        }
        report.results.push(TweakResult {
            tweak_id: spec.id.to_string(),
            name: spec.name.to_string(),
            ok,
            skipped: false,
            error: if errors.is_empty() {
                None
            } else {
                Some(errors.join("; "))
            },
        });
    }

    report.needs_admin = report.failed() > 0 && !system::is_admin();
    report
}

pub fn restore_snapshot(snapshot: &Snapshot) -> ApplyReport {
    let mut report = ApplyReport {
        label: format!("Restore snapshot: {}", snapshot.label),
        snapshot_id: Some(snapshot.id.clone()),
        ..Default::default()
    };
    if snapshot.entries.is_empty() {
        report.fatal = Some("This snapshot has no entries.".into());
        return report;
    }

    let current: Vec<SnapshotEntry> = snapshot.entries.iter().map(capture_current).collect();
    let _ = make_pre_restore_snapshot(
        &format!("Before restoring: {}", snapshot.label),
        &current,
    );

    for entry in snapshot.entries.iter().rev() {
        let errors = restore_entry(entry);
        let ok = errors.is_empty();
        if ok {
            logging::success(format!("Restored snapshot value: {}", entry.tweak_name));
        } else {
            logging::error(format!(
                "Snapshot restore failed: {} — {}",
                entry.tweak_name,
                errors.join("; ")
            ));
        }
        report.results.push(TweakResult {
            tweak_id: entry.tweak_id.clone(),
            name: entry.tweak_name.clone(),
            ok,
            skipped: false,
            error: if errors.is_empty() {
                None
            } else {
                Some(errors.join("; "))
            },
        });
    }
    report.needs_admin = report.failed() > 0 && !system::is_admin();
    report
}

pub fn restore_all_originals() -> ApplyReport {
    let originals = backup::load_originals();
    if originals.entries.is_empty() {
        return ApplyReport {
            label: "Restore all original settings".into(),
            fatal: Some(
                "The vault is empty. Clutchcore fills it the first time it changes a setting."
                    .into(),
            ),
            ..Default::default()
        };
    }

    let ids: Vec<String> = originals.entries.keys().cloned().collect();
    let current: Vec<SnapshotEntry> = originals
        .entries
        .values()
        .map(capture_current)
        .collect();
    let pre_snapshot = make_pre_restore_snapshot("Before restoring all original settings", &current);

    let mut report = ApplyReport {
        label: "Restore all original settings".into(),
        snapshot_id: pre_snapshot,
        ..Default::default()
    };

    for id in ids {
        let Some(entry) = originals.entries.get(&id) else {
            continue;
        };
        let errors = restore_entry(entry);
        let ok = errors.is_empty();
        if ok {
            logging::success(format!("Restored original: {}", entry.tweak_name));
        } else {
            logging::error(format!(
                "Could not restore {}: {}",
                entry.tweak_name,
                errors.join("; ")
            ));
        }
        report.results.push(TweakResult {
            tweak_id: entry.tweak_id.clone(),
            name: entry.tweak_name.clone(),
            ok,
            skipped: false,
            error: if errors.is_empty() {
                None
            } else {
                Some(errors.join("; "))
            },
        });
    }
    report.needs_admin = report.failed() > 0 && !system::is_admin();
    report
}

pub fn originals_count() -> usize {
    backup::load_originals().entries.len()
}

