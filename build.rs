use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/clutchcore.ico");
    println!("cargo:rerun-if-changed=assets/logo256.rgba");
    println!("cargo:rerun-if-changed=build.rs");

    println!("cargo:rerun-if-env-changed=RC_EXE");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let icon = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("assets")
        .join("clutchcore.ico");
    if !icon.exists() {
        println!("cargo:warning=assets/clutchcore.ico is missing, building without a file icon");
        return;
    }

    let Some(rc) = find_rc() else {
        println!("cargo:warning=rc.exe not found, building without a file icon");
        return;
    };

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let icon_copy = out.join("clutchcore.ico");
    if std::fs::copy(&icon, &icon_copy).is_err() {
        println!("cargo:warning=could not stage the icon, building without a file icon");
        return;
    }
    let script = out.join("clutchcore.rc");
    if std::fs::write(&script, RC_SOURCE).is_err() {
        println!("cargo:warning=could not write the resource script");
        return;
    }

    let status = Command::new(&rc)
        .current_dir(&out)
        .args(["/nologo", "/fo", "clutchcore.res", "clutchcore.rc"])
        .status();

    match status {
        Ok(status) if status.success() => {
            println!("cargo:rustc-link-arg={}", out.join("clutchcore.res").display());
        }
        _ => println!("cargo:warning=rc.exe failed, building without a file icon"),
    }
}

fn find_rc() -> Option<PathBuf> {
    if let Ok(explicit) = std::env::var("RC_EXE") {
        let path = PathBuf::from(explicit);
        if path.exists() {
            return Some(path);
        }
    }

    let mut best: Option<(Vec<u32>, PathBuf)> = None;
    for root in [
        r"C:\Program Files (x86)\Windows Kits\10\bin",
        r"C:\Program Files\Windows Kits\10\bin",
    ] {
        let Ok(entries) = std::fs::read_dir(root) else {
            continue;
        };
        for entry in entries.flatten() {
            let candidate = entry.path().join("x64").join("rc.exe");
            if !candidate.exists() {
                continue;
            }
            let version: Vec<u32> = entry
                .file_name()
                .to_string_lossy()
                .split('.')
                .filter_map(|part| part.parse().ok())
                .collect();
            if best
                .as_ref()
                .map(|(current, _)| version > *current)
                .unwrap_or(true)
            {
                best = Some((version, candidate));
            }
        }
    }
    best.map(|(_, path)| path)
}

const RC_SOURCE: &str = r#"IDI_ICON1 ICON "clutchcore.ico"

1 VERSIONINFO
FILEVERSION 1,0,4,0
PRODUCTVERSION 1,0,4,0
FILEFLAGSMASK 0x3fL
FILEFLAGS 0x0L
FILEOS 0x40004L
FILETYPE 0x1L
FILESUBTYPE 0x0L
BEGIN
    BLOCK "StringFileInfo"
    BEGIN
        BLOCK "040904b0"
        BEGIN
            VALUE "CompanyName", "Clutchcore"
            VALUE "FileDescription", "Clutchcore - competitive system tuner"
            VALUE "FileVersion", "1.0.4.0"
            VALUE "InternalName", "Clutchcore"
            VALUE "LegalCopyright", "Clutchcore"
            VALUE "OriginalFilename", "Clutchcore.exe"
            VALUE "ProductName", "Clutchcore"
            VALUE "ProductVersion", "1.0.4"
        END
    END
    BLOCK "VarFileInfo"
    BEGIN
        VALUE "Translation", 0x409, 1200
    END
END
"#;
