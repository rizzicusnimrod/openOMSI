//! Stamps the build with its version (`MAJOR.MINOR.COMMIT`, see docs/VERSIONING.md) and the
//! commit it came from, so a screenshot or a log line says which version is running; on
//! Windows it also puts the application icon into the executable.

use std::process::Command;

fn main() {
    let git = |args: &[&str]| -> Option<String> {
        let out = Command::new("git").args(args).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
    };
    let hash = git(&["rev-parse", "--short", "HEAD"]).unwrap_or_else(|| "unknown".into());
    let date = git(&["log", "-1", "--format=%cd", "--date=format:%Y-%m-%d %H:%M"]).unwrap_or_default();
    let dirty = git(&["status", "--porcelain"]).map(|s| !s.trim().is_empty()).unwrap_or(false);
    println!("cargo:rustc-env=OMSI_BUILD={hash}{} {date}", if dirty { "+" } else { "" });
    let version = version(&git);
    println!("cargo:rustc-env=OPENOMSI_VERSION={version}");
    println!("cargo:rerun-if-env-changed=OPENOMSI_VERSION");
    println!("cargo:rerun-if-changed=../../VERSION");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/index");
    windows_icon(&version);
    steam();
    // (the executable exports the two switchable-graphics hints of main.rs, see there)
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        for sym in ["NvOptimusEnablement", "AmdPowerXpressRequestHighPerformance"] {
            println!("cargo:rustc-link-arg-bin=openomsi=/EXPORT:{sym},DATA");
        }
    }
}

/// Steam's rich presence (`src/steam.rs`) on the targets `assets/steam_redist` has the library
/// for: `cfg(steam)`, and the library copied beside the binary, where the game looks for it
/// at its start (`@loader_path` on macOS, `$ORIGIN` on Linux, the exe's folder on Windows) -
/// without it there the game does not start at all. Android, Windows on ARM and Linux on ARM
/// have no Steam library and are built without it.
fn steam() {
    println!("cargo::rustc-check-cfg=cfg(steam)");
    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    let arch = std::env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let lib = match (os.as_str(), arch.as_str()) {
        ("windows", "x86_64") => "steam_api64.dll",
        ("linux", "x86_64") => "libsteam_api.so",
        ("macos", _) => "libsteam_api.dylib",
        _ => return,
    };
    println!("cargo:rustc-cfg=steam");
    if os == "linux" {
        println!("cargo:rustc-link-arg-bin=openomsi=-Wl,-rpath,$ORIGIN");
    }
    let src = std::path::Path::new("../../assets/steam_redist").join(lib);
    println!("cargo:rerun-if-changed={}", src.display());
    // (OUT_DIR is <target>/<profile>/build/<crate>/out: the binary lies three folders up)
    if let Some(dir) = std::env::var_os("OUT_DIR").map(std::path::PathBuf::from).and_then(|o| o.ancestors().nth(3).map(|d| d.to_path_buf())) {
        let _ = std::fs::copy(&src, dir.join(lib));
    }
}

/// `MAJOR.MINOR` from the VERSION file, then the number of commits since that file last
/// changed, not counting the never-released `[skip ci]` / `[skip actions]` ones (the CI
/// passes the same number in `OPENOMSI_VERSION`). A build of this fork is marked `-custom`:
/// the updater takes only the fork's releases for it (an official one would replace it).
fn version(git: &dyn Fn(&[&str]) -> Option<String>) -> String {
    if let Ok(v) = std::env::var("OPENOMSI_VERSION") {
        if !v.trim().is_empty() {
            return v.trim().to_string();
        }
    }
    let base = std::fs::read_to_string("../../VERSION").map(|s| s.trim().to_string()).unwrap_or_else(|_| "0.0".into());
    // (`:/` names the file from the top of the repository: git runs in this crate's folder,
    // where a plain `VERSION` is no file, and every local build was `.0`)
    let n = git(&["log", "-1", "--format=%H", "--", ":/VERSION"])
        .filter(|h| !h.is_empty())
        .and_then(|h| git(&["rev-list", "--count", "--invert-grep", "--fixed-strings", "--grep=[skip ci]", "--grep=[skip actions]", &format!("{h}..HEAD")]))
        .unwrap_or_else(|| "0".into());
    format!("{base}.{n}-custom")
}

/// The version as Windows keeps it in an executable: four numbers (`0.2.81-custom` is
/// 0.2.81.0). Without it Windows' crash and hang reports named every build 0.1.0.0.
fn file_version(version: &str) -> (String, u64) {
    let mut parts = [0u64; 4];
    for (slot, p) in parts.iter_mut().zip(version.split(['.', '-'])) {
        match p.parse::<u16>() {
            Ok(v) => *slot = v as u64,
            Err(_) => break,
        }
    }
    let text = parts.iter().map(|p| p.to_string()).collect::<Vec<_>>().join(".");
    (text, parts[0] << 48 | parts[1] << 32 | parts[2] << 16 | parts[3])
}

fn windows_icon(version: &str) {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    println!("cargo:rerun-if-changed=../../assets/icons/app/openomsi.ico");
    let (numbers, packed) = file_version(version);
    let mut res = winresource::WindowsResource::new();
    res.set_icon("../../assets/icons/app/openomsi.ico")
        .set("ProductName", "openOMSI")
        .set("FileDescription", "openOMSI")
        .set("ProductVersion", version)
        .set("FileVersion", &numbers)
        .set_version_info(winresource::VersionInfo::FILEVERSION, packed)
        .set_version_info(winresource::VersionInfo::PRODUCTVERSION, packed);
    if let Err(e) = res.compile() {
        println!("cargo:warning=no icon in the executable: {e}");
    }
}
