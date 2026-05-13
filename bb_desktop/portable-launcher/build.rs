use std::env;
use std::fs;
use std::path::PathBuf;

const EMPTY_PAYLOAD: &[u8] = b"BBPORTABLE1\n\0\0\0\0\0\0\0\0";

fn main() {
    println!("cargo:rerun-if-env-changed=BB_PORTABLE_PAYLOAD");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let payload = env::var_os("BB_PORTABLE_PAYLOAD")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            manifest_dir
                .join("..")
                .join("portable")
                .join("Blackboard-portable-payload.bbpack")
        });
    println!("cargo:rerun-if-changed={}", payload.display());

    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("payload.bbpack");
    if payload.is_file() {
        fs::copy(&payload, &out).expect("copy portable payload");
    } else {
        fs::write(&out, EMPTY_PAYLOAD).expect("write empty portable payload placeholder");
    }

    // Embed icon into the Windows executable
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let icon_path = manifest_dir
            .join("..")
            .join("src-tauri")
            .join("icons")
            .join("icon.ico");
        println!("cargo:rerun-if-changed={}", icon_path.display());
        if icon_path.is_file() {
            let mut res = winres::WindowsResource::new();
            res.set_icon(icon_path.to_str().expect("icon path"));
            res.compile().expect("failed to embed icon");
        }
    }
}
