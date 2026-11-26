use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/app.ico");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    // Embed icon, VersionInfo, and asInvoker manifest when targeting MSVC with rc.exe / Windows SDK.
    // On local gnullvm (or environments without Windows SDK rc.exe), cleanly no-op without failing.
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() != Ok("msvc") {
        return;
    }

    let Some(rc_exe) = find_rc_exe() else {
        return;
    };

    let Ok(manifest_dir) = env::var("CARGO_MANIFEST_DIR") else {
        return;
    };
    let Ok(out_dir) = env::var("OUT_DIR") else {
        return;
    };

    let ico_path = Path::new(&manifest_dir).join("assets").join("app.ico");
    if !ico_path.exists() {
        return;
    }

    let out_path = Path::new(&out_dir);
    let manifest_path = out_path.join("app.manifest");
    let rc_path = out_path.join("app.rc");
    let res_path = out_path.join("app.res");

    let manifest_xml = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <assemblyIdentity version="1.0.0.0" processorArchitecture="*" name="GokhanYucel.StollKalipEditoru" type="win32"/>
  <description>Stoll .shp / .shv Kalıp Editörü</description>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="asInvoker" uiAccess="false"/>
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>
"#;

    if std::fs::write(&manifest_path, manifest_xml.as_bytes()).is_err() {
        return;
    }

    let ico_escaped = ico_path.to_string_lossy().replace('\\', "\\\\");
    let manifest_escaped = manifest_path.to_string_lossy().replace('\\', "\\\\");

    let rc_text = format!(
        "1 ICON \"{ico_escaped}\"\r\n\
         1 24 \"{manifest_escaped}\"\r\n\
         1 VERSIONINFO\r\n\
         FILEVERSION 1,0,0,0\r\n\
         PRODUCTVERSION 1,0,0,0\r\n\
         FILEFLAGSMASK 0x3fL\r\n\
         FILEFLAGS 0x0L\r\n\
         FILEOS 0x40004L\r\n\
         FILETYPE 0x1L\r\n\
         FILESUBTYPE 0x0L\r\n\
         BEGIN\r\n\
           BLOCK \"StringFileInfo\"\r\n\
           BEGIN\r\n\
             BLOCK \"040904B0\"\r\n\
             BEGIN\r\n\
               VALUE \"CompanyName\", \"Gökhan Yücel\"\r\n\
               VALUE \"FileDescription\", \"Stoll .shp / .shv Kalıp Editörü\"\r\n\
               VALUE \"FileVersion\", \"1.0.0\"\r\n\
               VALUE \"InternalName\", \"Stoll_Kalip_Editoru\"\r\n\
               VALUE \"LegalCopyright\", \"Copyright (c) 2026 Gökhan\"\r\n\
               VALUE \"OriginalFilename\", \"Stoll_Kalip_Editoru.exe\"\r\n\
               VALUE \"ProductName\", \"Stoll Kalıp Editörü\"\r\n\
               VALUE \"ProductVersion\", \"1.0.0\"\r\n\
             END\r\n\
           END\r\n\
           BLOCK \"VarFileInfo\"\r\n\
           BEGIN\r\n\
             VALUE \"Translation\", 0x0409, 1200\r\n\
           END\r\n\
         END\r\n"
    );

    // Write UTF-16LE with BOM so rc.exe preserves Turkish Unicode strings cleanly.
    let mut utf16_bytes = Vec::with_capacity(2 + rc_text.len() * 2);
    utf16_bytes.extend_from_slice(&[0xFF, 0xFE]);
    for unit in rc_text.encode_utf16() {
        utf16_bytes.extend_from_slice(&unit.to_le_bytes());
    }
    if std::fs::write(&rc_path, &utf16_bytes).is_err() {
        return;
    }

    let status = Command::new(&rc_exe)
        .arg("/nologo")
        .arg("/fo")
        .arg(&res_path)
        .arg(&rc_path)
        .status();

    if let Ok(st) = status {
        if st.success() && res_path.exists() {
            println!("cargo:rustc-link-arg-bins={}", res_path.display());
        }
    }
}

fn find_rc_exe() -> Option<PathBuf> {
    if let Ok(path_var) = env::var("PATH") {
        for dir in env::split_paths(&path_var) {
            let candidate = dir.join("rc.exe");
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }

    if let Ok(sdk_dir) = env::var("WindowsSdkDir") {
        if let Some(found) = find_rc_in_kits_bin(&PathBuf::from(sdk_dir).join("bin")) {
            return Some(found);
        }
    }

    for root in [
        r"C:\Program Files (x86)\Windows Kits\10\bin",
        r"C:\Program Files\Windows Kits\10\bin",
    ] {
        if let Some(found) = find_rc_in_kits_bin(Path::new(root)) {
            return Some(found);
        }
    }

    None
}

fn find_rc_in_kits_bin(bin_dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(bin_dir).ok()?;
    let mut candidates: Vec<(String, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let p = entry.path();
        if p.is_dir() {
            let rc = p.join("x64").join("rc.exe");
            if rc.is_file() {
                let ver = entry.file_name().to_string_lossy().into_owned();
                candidates.push((ver, rc));
            }
        }
    }
    candidates.sort_by(|a, b| b.0.cmp(&a.0));
    candidates.into_iter().next().map(|(_, p)| p)
}
