use std::env;
use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=WindowsAppBlocker.exe.manifest");
    println!("cargo:rerun-if-env-changed=APP_BLOCKER_NO_MANIFEST");
    if env::var_os("CARGO_CFG_WINDOWS").is_none() {
        return;
    }

    // Embeds the manifest into the executable only (not into test binaries), so the app asks
    // for administrator rights when opened while `cargo test` still runs unelevated.
    // APP_BLOCKER_NO_MANIFEST=1 skips it, to preview the UI without elevation during development.
    if env::var_os("APP_BLOCKER_NO_MANIFEST").is_none() {
        embed_manifest::embed_manifest_file("WindowsAppBlocker.exe.manifest").expect("unable to embed WindowsAppBlocker.exe.manifest");
    }

    // The self-contained MinGW toolchain has no import library for shlwapi (used by a
    // dependency to open links). GNU ld can link against the DLL itself, so expose a copy of it
    // in a private search folder.
    if env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
        let system_root = env::var_os("SystemRoot").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Windows"));
        let out = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR")).join("dll-imports");
        std::fs::create_dir_all(&out).expect("create dll-imports");
        std::fs::copy(system_root.join("System32").join("shlwapi.dll"), out.join("shlwapi.dll")).expect("copy shlwapi.dll");
        println!("cargo:rustc-link-search=native={}", out.display());
    }
}
