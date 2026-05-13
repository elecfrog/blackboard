#![cfg_attr(target_os = "windows", windows_subsystem = "windows")]

use std::env;
use std::ffi::c_void;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const PAYLOAD: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/payload.bbpack"));
const MAGIC: &[u8] = b"BBPORTABLE1\n";
const READY_FILE: &str = ".blackboard-portable-ready";
const APP_EXE: &str = "blackboard-desktop.exe";

fn main() {
    if let Err(err) = run() {
        show_error(&format!("Blackboard Portable could not start.\n\n{err}"));
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let payload_hash = fnv1a64(PAYLOAD);
    let install_dir = portable_cache_dir()?.join(format!("{payload_hash:016x}"));
    let app_exe = install_dir.join(APP_EXE);

    if !install_dir.join(READY_FILE).is_file() || !app_exe.is_file() {
        unpack_fresh(&install_dir, payload_hash)?;
    }

    Command::new(&app_exe)
        .current_dir(&install_dir)
        .spawn()
        .map_err(|err| format!("failed to launch {}: {err}", app_exe.display()))?;

    Ok(())
}

fn portable_cache_dir() -> Result<PathBuf, String> {
    if let Some(local_app_data) = env::var_os("LOCALAPPDATA") {
        return Ok(PathBuf::from(local_app_data)
            .join("Blackboard")
            .join("portable"));
    }

    Ok(env::temp_dir().join("Blackboard").join("portable"))
}

fn unpack_fresh(install_dir: &Path, payload_hash: u64) -> Result<(), String> {
    if let Some(parent) = install_dir.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create portable cache directory {}: {err}",
                parent.display()
            )
        })?;
    }

    if install_dir.exists() {
        fs::remove_dir_all(install_dir).map_err(|err| {
            format!(
                "failed to refresh portable cache directory {}: {err}",
                install_dir.display()
            )
        })?;
    }
    fs::create_dir_all(install_dir).map_err(|err| {
        format!(
            "failed to create portable cache directory {}: {err}",
            install_dir.display()
        )
    })?;

    unpack_payload(PAYLOAD, install_dir)?;
    fs::write(
        install_dir.join(READY_FILE),
        format!("payload_hash={payload_hash:016x}\n"),
    )
    .map_err(|err| format!("failed to write portable ready marker: {err}"))?;
    Ok(())
}

fn unpack_payload(payload: &[u8], install_dir: &Path) -> Result<(), String> {
    let mut cursor = 0;
    let magic = take(payload, &mut cursor, MAGIC.len())?;
    if magic != MAGIC {
        return Err("portable payload has an invalid header".to_string());
    }

    let file_count = read_u64(payload, &mut cursor)?;
    if file_count == 0 {
        return Err(
            "portable payload is empty; rebuild with `python scripts\\desktop.py portable`"
                .to_string(),
        );
    }

    for _ in 0..file_count {
        let path_len = read_u32(payload, &mut cursor)? as usize;
        let file_len = read_u64(payload, &mut cursor)? as usize;
        let path_bytes = take(payload, &mut cursor, path_len)?;
        let file_bytes = take(payload, &mut cursor, file_len)?;
        let relative_path = std::str::from_utf8(path_bytes)
            .map_err(|err| format!("portable payload path is not UTF-8: {err}"))?;
        let output_path = safe_output_path(install_dir, relative_path)?;

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;
        }
        fs::write(&output_path, file_bytes)
            .map_err(|err| format!("failed to write {}: {err}", output_path.display()))?;
    }

    Ok(())
}

fn safe_output_path(base: &Path, relative_path: &str) -> Result<PathBuf, String> {
    let mut output = base.to_path_buf();
    for part in relative_path.split('/') {
        if part.is_empty()
            || part == "."
            || part == ".."
            || part.contains('\\')
            || part.contains(':')
        {
            return Err(format!(
                "portable payload contains unsafe path: {relative_path}"
            ));
        }
        output.push(part);
    }
    Ok(output)
}

fn take<'a>(payload: &'a [u8], cursor: &mut usize, len: usize) -> Result<&'a [u8], String> {
    let end = cursor
        .checked_add(len)
        .ok_or_else(|| "portable payload cursor overflow".to_string())?;
    if end > payload.len() {
        return Err("portable payload is truncated".to_string());
    }
    let bytes = &payload[*cursor..end];
    *cursor = end;
    Ok(bytes)
}

fn read_u32(payload: &[u8], cursor: &mut usize) -> Result<u32, String> {
    let bytes = take(payload, cursor, 4)?;
    Ok(u32::from_le_bytes(
        bytes.try_into().expect("u32 slice length checked"),
    ))
}

fn read_u64(payload: &[u8], cursor: &mut usize) -> Result<u64, String> {
    let bytes = take(payload, cursor, 8)?;
    Ok(u64::from_le_bytes(
        bytes.try_into().expect("u64 slice length checked"),
    ))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(target_os = "windows")]
fn show_error(message: &str) {
    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(
            hwnd: *mut c_void,
            text: *const u16,
            caption: *const u16,
            message_type: u32,
        ) -> i32;
    }

    let text: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
    let caption: Vec<u16> = "Blackboard Portable"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            text.as_ptr(),
            caption.as_ptr(),
            0x0000_0010,
        );
    }
}

#[cfg(not(target_os = "windows"))]
fn show_error(message: &str) {
    eprintln!("{message}");
}
