use std::path::PathBuf;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=../assets/clutchcore.ico");
    println!("cargo:rerun-if-env-changed=RC_EXE");

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("OUT_DIR"));
    let app = std::env::var_os("CLUTCHCORE_APP_EXE")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest.join("..").join("dist").join("Clutchcore.exe"));
    println!("cargo:rerun-if-changed={}", app.display());
    println!("cargo:rerun-if-env-changed=CLUTCHCORE_APP_EXE");

    let bytes = std::fs::read(&app).unwrap_or_else(|error| {
        panic!(
            "\n\n  Clutchcore.exe was not found in dist/ ({error}).\n\n  Build it first:\n    cd Clutchcore && cargo build --release\n    cp target/release/Clutchcore.exe dist/Clutchcore.exe\n\n  Or point CLUTCHCORE_APP_EXE at the built binary.\n\n"
        )
    });

    let packed = miniz_oxide::deflate::compress_to_vec(&bytes, 9);
    let mut payload = Vec::with_capacity(8 + packed.len());
    payload.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    payload.extend_from_slice(&packed);
    std::fs::write(out.join("payload.bin"), &payload).expect("write payload");

    println!(
        "cargo:warning=payload: {} bytes -> {} bytes embedded",
        bytes.len(),
        payload.len()
    );

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let icon = manifest
        .join("..")
        .join("assets")
        .join("clutchcore.ico");
    if !icon.exists() {
        println!("cargo:warning=assets/clutchcore.ico is missing, setup will use the default icon");
        return;
    }
    let Some(rc) = find_rc() else {
        println!("cargo:warning=rc.exe not found, setup will use the default icon");
        return;
    };

    if std::fs::copy(&icon, out.join("clutchcore.ico")).is_err() {
        println!("cargo:warning=could not stage the icon");
        return;
    }
    if std::fs::write(out.join("setup.rc"), RC_SOURCE).is_err() {
        println!("cargo:warning=could not write the resource script");
        return;
    }

    let status = Command::new(&rc)
        .current_dir(&out)
        .args(["/nologo", "/fo", "setup.res", "setup.rc"])
        .status();

    match status {
        Ok(status) if status.success() => {
            println!("cargo:rustc-link-arg={}", out.join("setup.res").display());
        }
        _ => println!("cargo:warning=rc.exe failed, setup will use the default icon"),
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
            VALUE "FileDescription", "Clutchcore Setup"
            VALUE "FileVersion", "1.0.4.0"
            VALUE "InternalName", "ClutchcoreSetup"
            VALUE "LegalCopyright", "Clutchcore"
            VALUE "OriginalFilename", "Clutchcore-Setup-1.0.4.exe"
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
