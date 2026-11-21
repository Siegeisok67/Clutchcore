use crate::analysis;
use crate::backup;
use crate::engine;
use crate::model::*;
use crate::network;
use crate::presets;
use crate::profiles;
use crate::registry;
use crate::system;
use crate::tweaks;
use crate::util;

pub fn run(args: &[String]) -> bool {
    if args.is_empty() {
        return false;
    }
    match args[0].as_str() {
        "--help" | "-h" | "/?" => {
            print_help();
            true
        }
        "--version" | "-V" => {
            println!("Clutchcore {}", env!("CARGO_PKG_VERSION"));
            true
        }
        "--scan" => {
            scan_command();
            true
        }
        "--tweaks" => {
            tweaks_command();
            true
        }
        "--profiles" => {
            profiles_command();
            true
        }
        "--probe" => {
            probe_command(args.get(1).map(|s| s.as_str()));
            true
        }
        "--apply-profile" => {
            let Some(id) = args.get(1) else {
                println!("Usage: Clutchcore.exe --apply-profile <profile-id>");
                return true;
            };
            apply_command(id);
            true
        }
        "--restore-all" => {
            restore_command();
            true
        }
        "--selftest" => {
            selftest_command();
            true
        }
        other => {
            println!("Unknown argument: {other}\n");
            print_help();
            true
        }
    }
}

fn print_help() {
    println!(
        "Clutchcore {}\n\
         Competitive system tuner for Windows.\n\n\
         Usage:\n\
           Clutchcore.exe                     Open the app\n\
           Clutchcore.exe --scan              Scan specs, tweak status and suggestions\n\
           Clutchcore.exe --tweaks            List every tweak and its current state\n\
           Clutchcore.exe --profiles          List game and custom profiles\n\
           Clutchcore.exe --probe [host]      Test regional routes, or one custom host\n\
           Clutchcore.exe --apply-profile ID  Apply a profile (writes a backup first)\n\
           Clutchcore.exe --restore-all       Restore every captured original value\n\
           Clutchcore.exe --selftest          Non-destructive engine self-check\n\n\
         Data folder: {}\n",
        env!("CARGO_PKG_VERSION"),
        util::data_dir().display()
    );
}

fn scan_command() {
    let report = system::scan();
    println!("{}", system::report_text(&report));
    let detection = tweaks::detect_all();
    let results = network::load_results();
    let summary = network::summarize(&results);
    let readiness = analysis::readiness(&detection.statuses, &report, summary.as_ref());
    println!("\nCOMPETITIVE READINESS\n");
    println!("Overall: {}", readiness.overall);
    println!(
        "Input {} · Latency {} · FPS {} · Network {}",
        readiness.input, readiness.latency, readiness.fps, readiness.network
    );
    println!("\nSUGGESTIONS\n");
    for suggestion in analysis::suggestions(&detection.statuses, &report, summary.as_ref(), report.admin)
    {
        println!(
            "- [{}] {} — {}",
            suggestion.severity.label(),
            suggestion.title,
            suggestion.detail
        );
    }
}

fn tweaks_command() {
    let detection = tweaks::detect_all();
    println!("CLUTCHCORE TWEAKS\n");
    for spec in tweaks::specs() {
        let status = detection.statuses.get(spec.id);
        let mark = match status.map(|s| s.state) {
            Some(TweakState::Applied) => "[x]",
            Some(TweakState::Partial) => "[~]",
            Some(TweakState::Unknown) => "[?]",
            _ => "[ ]",
        };
        let mut flags = Vec::new();
        if spec.requires_admin {
            flags.push("admin");
        }
        if spec.requires_reboot {
            flags.push("reboot");
        }
        if spec.risk != Risk::Low {
            flags.push(spec.risk.label());
        }
        println!(
            "{mark} {:<38} {:<10} {}{}",
            spec.name,
            spec.category.label(),
            status.map(|s| s.detail.as_str()).unwrap_or("not checked"),
            if flags.is_empty() {
                String::new()
            } else {
                format!("  [{}]", flags.join(", "))
            }
        );
    }
    let originals = backup::load_originals();
    let snapshots = backup::list_snapshots();
    println!(
        "\nOriginal values captured: {} tweak(s)\nSnapshots on disk: {}\nData folder: {}",
        originals.entries.len(),
        snapshots.len(),
        util::data_dir().display()
    );
}

fn profiles_command() {
    let store = profiles::load_store();
    println!("GAME PRESETS\n");
    for profile in profiles::builtin() {
        if profile.game.is_some() {
            println!(
                "- {:<28} {} tweaks · {}",
                profile.name,
                profile.tweak_ids.len(),
                profile.description
            );
        }
    }
    println!("\nSYSTEM PRESETS\n");
    for profile in profiles::builtin() {
        if profile.game.is_none() {
            println!(
                "- {:<28} {} tweaks · {}",
                profile.name,
                profile.tweak_ids.len(),
                profile.description
            );
        }
    }
    println!("\nCUSTOM PROFILES\n");
    if store.profiles.is_empty() {
        println!("(none yet)");
    }
    for profile in &store.profiles {
        println!(
            "- {:<28} id={} · {} tweaks{}",
            profile.name,
            profile.id,
            profile.tweak_ids.len(),
            if store.active.as_deref() == Some(profile.id.as_str()) {
                " · ACTIVE"
            } else {
                ""
            }
        );
    }
    println!("\nApply with: Clutchcore.exe --apply-profile <id>");
}

fn apply_command(id: &str) {
    let store = profiles::load_store();
    let Some(profile) = profiles::find(&store, id) else {
        println!("Profile '{id}' was not found. Run --profiles to list ids.");
        return;
    };
    let ids = profiles::profile_tweak_ids(&profile);
    println!(
        "Applying '{}' ({} tweaks). A snapshot is written before anything changes…\n",
        profile.name,
        ids.len()
    );
    let report = engine::apply(&ids, &format!("{} preset (CLI)", profile.name), Some(&profile.id));
    print_report(&report);
    if report.failed() == 0 {
        let mut store = profiles::load_store();
        let _ = profiles::set_active(&mut store, Some(&profile.id));
    }
    if report.needs_reboot {
        println!("\nA reboot is required for at least one tweak.");
    }
}

fn restore_command() {
    println!("Restoring every captured original value. A pre-restore snapshot is written first…\n");
    let report = engine::restore_all_originals();
    print_report(&report);
}

fn print_report(report: &engine::ApplyReport) {
    for result in &report.results {
        let mark = if result.ok {
            if result.skipped {
                "SKIP"
            } else {
                "OK"
            }
        } else {
            "FAIL"
        };
        println!(
            "{:<5} {}{}",
            mark,
            result.name,
            result
                .error
                .as_ref()
                .map(|e| format!(" — {e}"))
                .unwrap_or_default()
        );
    }
    println!("\n{}", report.summary());
    if let Some(id) = &report.snapshot_id {
        println!("Backup snapshot: {id}");
    }
    println!("Data folder: {}", util::data_dir().display());
}

fn probe_command(host: Option<&str>) {
    if let Some(host) = host {
        println!("Probing {host}…");
        let result = network::custom_probe(host, 8);
        print_probe(&result);
        return;
    }

    println!("Probing {} regional and provider gateways…\n", network::endpoints().len());
    let mut handles = Vec::new();
    for endpoint in network::endpoints() {
        handles.push(std::thread::spawn(move || {
            network::probe(
                endpoint.label,
                endpoint.provider,
                endpoint.region,
                endpoint.host,
                endpoint.port,
                8,
                1000,
            )
        }));
    }
    let mut results: Vec<network::ProbeResult> = handles
        .into_iter()
        .filter_map(|handle| handle.join().ok())
        .collect();
    results.sort_by(|a, b| {
        a.avg_ms
            .partial_cmp(&b.avg_ms)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    for result in &results {
        print_probe(result);
    }
    network::save_results(&results);
    if let Some(summary) = network::summarize(&results) {
        println!(
            "\nBest route: {} · {:.1} ms · route jitter {:.1} ms (average across routes {:.1} ms)",
            summary.best_label, summary.best_ms, summary.best_jitter, summary.avg_jitter
        );
    }
    println!("Results cached in {}", network::cache_path().display());
}

fn print_probe(result: &network::ProbeResult) {
    match (&result.error, result.samples.is_empty()) {
        (Some(error), _) => println!("{:<24} FAILED — {error}", result.label),
        (None, false) => println!(
            "{:<24} {:>7.1} ms  jitter {:>5.1}  loss {:>4.0}%  {}",
            result.label,
            result.avg_ms,
            result.jitter_ms,
            result.loss_pct,
            result.ip.clone().unwrap_or_default()
        ),
        _ => println!("{:<24} no samples", result.label),
    }
}

fn selftest_command() {
    let mut failures = 0usize;
    macro_rules! check {
        ($name:expr, $cond:expr) => {
            if $cond {
                println!("PASS  {}", $name);
            } else {
                println!("FAIL  {}", $name);
                failures += 1;
            }
        };
    }

    println!("CLUTCHCORE SELF-TEST\n");
    let encoded = util::hex_encode(&[0xDE, 0xAD, 0xBE, 0xEF]);
    check!("hex encode", encoded == "deadbeef");
    check!(
        "hex decode",
        util::hex_decode(&encoded).as_deref() == Some(&[0xDE, 0xAD, 0xBE, 0xEF][..])
    );
    check!(
        "dword value round-trip",
        registry::RawValue::dword(42).to_u32() == Some(42)
    );
    check!(
        "string value round-trip",
        registry::RawValue::string("hello")
            .to_string_value()
            .as_deref()
            == Some("hello")
    );

    let base = r"Software\Clutchcore\SelfTest";
    match registry::RegBackup::capture(Hive::Hkcu, base, "Probe") {
        Ok(backup) => {
            check!("registry scratch backup", !backup.existed);
            let write = registry::write_desired(
                Hive::Hkcu,
                base,
                "Probe",
                DesiredValue::Dword(4242),
            );
            check!("registry scratch write", write.is_ok());
            let read_back = registry::read(Hive::Hkcu, base, "Probe")
                .ok()
                .flatten()
                .and_then(|v| v.to_u32());
            check!("registry scratch read", read_back == Some(4242));
            let restored = backup.restore();
            check!("registry scratch restore", restored.is_ok());
            let after = registry::read(Hive::Hkcu, base, "Probe").ok().flatten();
            check!("registry scratch cleanup", after.is_none());
        }
        Err(error) => {
            println!("FAIL  registry scratch backup — {error}");
            failures += 1;
        }
    }
    let _ = winreg::RegKey::predef(winreg::enums::HKEY_CURRENT_USER).delete_subkey_all(base);

    check!("game preset count", presets::all().len() >= 10);
    let mut preset_ids_ok = true;
    for preset in presets::all() {
        if preset.game.is_empty()
            || preset.in_game.is_empty()
            || preset
                .tweak_ids
                .iter()
                .any(|id| tweaks::find(id).is_none())
        {
            preset_ids_ok = false;
            println!("      broken preset: {}", preset.id);
        }
    }
    check!("preset tweak ids resolve", preset_ids_ok);

    let builtin = profiles::builtin();
    let unique = {
        let mut ids: Vec<&str> = builtin.iter().map(|p| p.id.as_str()).collect();
        ids.sort();
        ids.dedup();
        ids.len() == builtin.len()
    };
    check!("profile ids are unique", !builtin.is_empty() && unique);

    let store = profiles::ProfileStore::default();
    let json = serde_json::to_string_pretty(&store);
    check!(
        "profile store JSON round-trip",
        json.is_ok()
            && json
                .ok()
                .and_then(|text| serde_json::from_str::<profiles::ProfileStore>(&text).ok())
                .is_some()
    );

    let report = system::scan();
    check!("system scan: CPU name", !report.cpu_name.trim().is_empty());
    check!("system scan: logical cores", report.logical_cores > 0);
    check!("system scan: memory", report.ram_total_mb > 0);
    println!(
        "      {} · {} cores / {} threads · {} MB RAM · {} display(s) · {} game(s) detected",
        report.cpu_name,
        report.physical_cores,
        report.logical_cores,
        report.ram_total_mb,
        report.displays.len(),
        report.detected_games.len()
    );

    let detection = tweaks::detect_all();
    check!(
        "tweak detection",
        detection.statuses.len() == tweaks::specs().len()
    );

    check!(
        "network target parsing",
        network::parse_target("example.com:1234").ok() == Some(("example.com".to_string(), 1234))
    );
    check!(
        "network target default port",
        network::parse_target("example.com").ok() == Some(("example.com".to_string(), 443))
    );

    let snapshot = backup::Snapshot {
        id: "selftest".into(),
        created_unix: util::unix_now(),
        label: "Self-test".into(),
        profile: None,
        entries: Vec::new(),
        results: Vec::new(),
        restored: false,
        app_version: env!("CARGO_PKG_VERSION").into(),
    };
    let snapshot_json = serde_json::to_string(&snapshot);
    check!(
        "snapshot JSON round-trip",
        snapshot_json.is_ok()
            && snapshot_json
                .ok()
                .and_then(|text| serde_json::from_str::<backup::Snapshot>(&text).ok())
                .is_some()
    );

    println!("\n{} failure(s)", failures);
    if failures > 0 {
        std::process::exit(1);
    }
}
