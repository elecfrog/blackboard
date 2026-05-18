//! 平台相关工具函数（主机名、OS 版本等）。

use std::fs;
#[cfg(windows)]
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;

/// 获取当前机器主机名。
#[must_use]
pub fn machine_host_name() -> Option<String> {
    std::env::var("COMPUTERNAME")
        .or_else(|_| std::env::var("HOSTNAME"))
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| command_output("hostname", &["-s"]))
}

/// 获取用户 home 目录。
pub fn user_home_dir() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)
}

/// 获取当前操作系统名称。
#[must_use]
pub fn current_os_name() -> &'static str {
    match std::env::consts::OS {
        "windows" => "Windows",
        "macos" => "MacOS",
        "linux" => "Linux",
        other => other,
    }
}

/// 获取当前操作系统版本。
#[must_use]
pub fn current_os_version() -> Option<String> {
    match std::env::consts::OS {
        "windows" => windows_major_version(),
        "macos" => command_output("sw_vers", &["-productVersion"]),
        "linux" => linux_version_id(),
        _ => None,
    }
}

fn windows_major_version() -> Option<String> {
    let output = command_output(
        "reg",
        &[
            "query",
            r"HKLM\SOFTWARE\Microsoft\Windows NT\CurrentVersion",
            "/v",
            "CurrentBuildNumber",
        ],
    )?;
    let build = output
        .split_whitespace()
        .last()
        .and_then(|value| value.parse::<u32>().ok())?;
    if build >= 22_000 {
        Some("11".to_string())
    } else {
        Some("10".to_string())
    }
}

fn linux_version_id() -> Option<String> {
    let contents = fs::read_to_string("/etc/os-release").ok()?;
    for line in contents.lines() {
        let Some(version) = line.strip_prefix("VERSION_ID=") else {
            continue;
        };
        return Some(version.trim_matches('"').to_string());
    }
    None
}

#[must_use]
pub fn command_output(command: &str, args: &[&str]) -> Option<String> {
    let output = std::process::Command::new(command)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

/// Resolve a command name into a path that `std::process::Command` can spawn.
///
/// On Windows, `Command::new("opencode")` does not use PowerShell/Git Bash
/// command discovery, and npm shims often resolve to `.cmd`, `.ps1`, or a
/// shell script that `CreateProcess` cannot execute directly. Prefer native
/// executables and unwrap known npm package layouts when possible.
#[must_use]
pub fn resolve_spawn_program(program: &str) -> String {
    #[cfg(windows)]
    {
        resolve_spawn_program_windows(program)
    }

    #[cfg(not(windows))]
    {
        program.to_string()
    }
}

/// Resolve Pi into a spawnable command.
///
/// On Windows, Pi is commonly installed as an npm shim. Spawning `pi.cmd`
/// directly can fail for arbitrary prompt text because batch-file argument
/// escaping is restricted by Rust/Windows. Prefer `node <pi cli.js>` so prompt
/// arguments are passed to a native executable.
#[must_use]
pub fn resolve_pi_spawn_command(program: &str) -> (String, Vec<String>) {
    #[cfg(windows)]
    {
        resolve_pi_spawn_command_windows(program)
            .unwrap_or_else(|| (resolve_spawn_program(program), Vec::new()))
    }

    #[cfg(not(windows))]
    {
        (program.to_string(), Vec::new())
    }
}

/// Resolve Codex into a spawnable command.
///
/// On Windows, npm installs Codex as a `.cmd`/`.ps1` shim. Spawning the batch
/// shim directly can fail once the prompt/config arguments become complex, so
/// prefer `node <@openai/codex/bin/codex.js>` when that package layout exists.
#[must_use]
pub fn resolve_codex_spawn_command(program: &str) -> (String, Vec<String>) {
    #[cfg(windows)]
    {
        resolve_codex_spawn_command_windows(program)
            .unwrap_or_else(|| (resolve_spawn_program(program), Vec::new()))
    }

    #[cfg(not(windows))]
    {
        (program.to_string(), Vec::new())
    }
}

/// Terminate a child process and, on Windows, its descendants.
///
/// npm-backed CLIs often spawn a native child process that inherits stdout and
/// stderr. Killing only the wrapper can leave the real runtime alive and keep
/// pipe reader threads blocked past the configured timeout.
pub fn terminate_child_process_tree(child: &mut Child) -> std::io::Result<()> {
    #[cfg(windows)]
    {
        let status = std::process::Command::new("taskkill")
            .arg("/PID")
            .arg(child.id().to_string())
            .arg("/T")
            .arg("/F")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status();
        match status {
            Ok(status) if status.success() => Ok(()),
            _ => child.kill(),
        }
    }

    #[cfg(not(windows))]
    {
        child.kill()
    }
}

#[cfg(windows)]
fn resolve_spawn_program_windows(program: &str) -> String {
    let trimmed = program.trim();
    if trimmed.is_empty() {
        return program.to_string();
    }

    let path = Path::new(trimmed);
    if path.extension().is_some() {
        return resolve_windows_program_path(path).unwrap_or_else(|| program.to_string());
    }

    if path.components().count() > 1 || trimmed.contains('\\') || trimmed.contains('/') {
        return resolve_windows_program_path(path).unwrap_or_else(|| program.to_string());
    }

    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            if let Some(resolved) = resolve_windows_candidate(&dir.join(trimmed)) {
                return resolved;
            }
        }
    }

    program.to_string()
}

#[cfg(windows)]
fn resolve_codex_spawn_command_windows(program: &str) -> Option<(String, Vec<String>)> {
    let command_path = find_windows_command_candidate(program.trim())?;
    let bin_dir = command_path.parent()?;
    let cli = bin_dir
        .join("node_modules")
        .join("@openai")
        .join("codex")
        .join("bin")
        .join("codex.js");
    if !cli.is_file() {
        return None;
    }

    let local_node = bin_dir.join("node.exe");
    let node = if local_node.is_file() {
        local_node.to_string_lossy().to_string()
    } else {
        resolve_spawn_program("node")
    };

    Some((node, vec![cli.to_string_lossy().to_string()]))
}

#[cfg(windows)]
fn resolve_pi_spawn_command_windows(program: &str) -> Option<(String, Vec<String>)> {
    let command_path = find_windows_command_candidate(program.trim())?;
    let bin_dir = command_path.parent()?;
    let cli = bin_dir
        .join("node_modules")
        .join("@earendil-works")
        .join("pi-coding-agent")
        .join("dist")
        .join("cli.js");
    if !cli.is_file() {
        return None;
    }

    let local_node = bin_dir.join("node.exe");
    let node = if local_node.is_file() {
        local_node.to_string_lossy().to_string()
    } else {
        resolve_spawn_program("node")
    };

    Some((node, vec![cli.to_string_lossy().to_string()]))
}

#[cfg(windows)]
fn find_windows_command_candidate(program: &str) -> Option<PathBuf> {
    if program.is_empty() {
        return None;
    }

    let path = Path::new(program);
    let has_path_segment =
        path.components().count() > 1 || program.contains('\\') || program.contains('/');
    if has_path_segment {
        if path.is_file() {
            return Some(path.to_path_buf());
        }
        for ext in windows_spawn_extensions() {
            let candidate = with_windows_extension(path, &ext);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        return None;
    }

    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            for ext in windows_spawn_extensions() {
                let candidate = with_windows_extension(&dir.join(program), &ext);
                if candidate.is_file() {
                    return Some(candidate);
                }
            }
        }
    }

    None
}

#[cfg(windows)]
fn resolve_windows_program_path(path: &Path) -> Option<String> {
    if path.is_file() {
        if is_native_windows_executable(path) {
            return Some(path.to_string_lossy().to_string());
        }
        if let Some(resolved) = resolve_windows_shim_target(path) {
            return Some(resolved);
        }
        if is_windows_command_script(path) {
            return Some(path.to_string_lossy().to_string());
        }
    }

    resolve_windows_candidate(path)
}

#[cfg(windows)]
fn resolve_windows_candidate(base: &Path) -> Option<String> {
    for ext in windows_spawn_extensions() {
        let candidate = with_windows_extension(base, &ext);
        if !candidate.is_file() {
            continue;
        }
        if is_native_windows_executable(&candidate) {
            return Some(candidate.to_string_lossy().to_string());
        }
        if let Some(resolved) = resolve_windows_shim_target(&candidate) {
            return Some(resolved);
        }
        if is_windows_command_script(&candidate) {
            return Some(candidate.to_string_lossy().to_string());
        }
    }

    None
}

#[cfg(windows)]
fn windows_spawn_extensions() -> Vec<String> {
    let mut exts = vec![String::new(), ".com".to_string(), ".exe".to_string()];
    if let Ok(path_ext) = std::env::var("PATHEXT") {
        for ext in path_ext.split(';') {
            let ext = normalize_windows_extension(ext);
            if is_supported_windows_extension(&ext) && !exts.iter().any(|seen| seen == &ext) {
                exts.push(ext);
            }
        }
    }

    for ext in [".com", ".exe", ".bat", ".cmd", ".ps1"] {
        if !exts.iter().any(|seen| seen == ext) {
            exts.push(ext.to_string());
        }
    }

    exts
}

#[cfg(windows)]
fn normalize_windows_extension(ext: &str) -> String {
    let trimmed = ext.trim().trim_matches('"').to_ascii_lowercase();
    if trimmed.is_empty() {
        String::new()
    } else if trimmed.starts_with('.') {
        trimmed
    } else {
        format!(".{trimmed}")
    }
}

#[cfg(windows)]
fn is_supported_windows_extension(ext: &str) -> bool {
    matches!(ext, ".com" | ".exe" | ".bat" | ".cmd" | ".ps1")
}

#[cfg(windows)]
fn is_native_windows_executable(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            let ext = ext.to_ascii_lowercase();
            ext == "exe" || ext == "com"
        })
}

#[cfg(windows)]
fn is_windows_command_script(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| {
            let ext = ext.to_ascii_lowercase();
            ext == "bat" || ext == "cmd"
        })
}

#[cfg(windows)]
fn resolve_windows_shim_target(shim_path: &Path) -> Option<String> {
    if let Some(resolved) = resolve_opencode_npm_binary(shim_path) {
        return Some(resolved);
    }

    let content = fs::read_to_string(shim_path).ok()?;
    let script_dir = shim_path.parent()?;
    let shim_stem = shim_path
        .file_stem()?
        .to_string_lossy()
        .to_ascii_lowercase();

    for line in content.lines() {
        for quoted in quoted_segments(line) {
            let expanded = expand_batch_script_dir(&quoted, script_dir);
            let candidate = PathBuf::from(expanded);
            let candidate_stem = candidate
                .file_stem()
                .map(|stem| stem.to_string_lossy().to_ascii_lowercase());
            if candidate_stem.as_deref() != Some(shim_stem.as_str()) {
                continue;
            }
            if is_native_windows_executable(&candidate) && candidate.is_file() {
                return Some(candidate.to_string_lossy().to_string());
            }
        }
    }

    None
}

#[cfg(windows)]
fn resolve_opencode_npm_binary(shim_path: &Path) -> Option<String> {
    let stem = shim_path
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_ascii_lowercase())?;
    if stem != "opencode" {
        return None;
    }
    let dir = shim_path.parent()?;
    for candidate in [
        dir.join("node_modules/opencode-ai/bin/opencode.exe"),
        dir.join("node_modules/opencode-ai/node_modules/opencode-windows-x64/bin/opencode.exe"),
        dir.join(
            "node_modules/opencode-ai/node_modules/opencode-windows-x64-baseline/bin/opencode.exe",
        ),
    ] {
        if candidate.is_file() {
            return Some(candidate.to_string_lossy().to_string());
        }
    }
    None
}

#[cfg(windows)]
fn quoted_segments(line: &str) -> Vec<String> {
    let mut segments = Vec::new();
    let mut rest = line;
    while let Some(start) = rest.find('"') {
        let after_start = &rest[start + 1..];
        let Some(end) = after_start.find('"') else {
            break;
        };
        segments.push(after_start[..end].to_string());
        rest = &after_start[end + 1..];
    }
    segments
}

#[cfg(windows)]
fn expand_batch_script_dir(value: &str, script_dir: &Path) -> String {
    let dir = script_dir.to_string_lossy();
    let dir_with_slash = if dir.ends_with('\\') || dir.ends_with('/') {
        dir.to_string()
    } else {
        format!("{dir}\\")
    };

    value
        .replace("%dp0%", &dir)
        .replace("%~dp0", &dir_with_slash)
        .replace("$basedir", &dir)
}

#[cfg(windows)]
fn with_windows_extension(base: &Path, ext: &str) -> PathBuf {
    let mut candidate = base.as_os_str().to_os_string();
    candidate.push(ext);
    PathBuf::from(candidate)
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use std::path::Path;
    #[cfg(windows)]
    use std::sync::{Mutex, OnceLock};

    #[cfg(windows)]
    use super::{resolve_codex_spawn_command, resolve_pi_spawn_command, resolve_spawn_program};

    #[cfg(windows)]
    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    #[cfg(windows)]
    fn resolve_spawn_program_unwraps_opencode_1150_npm_shim() {
        let _guard = env_lock().lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("bin");
        let target_dir = bin
            .join("node_modules")
            .join("opencode-ai")
            .join("node_modules")
            .join("opencode-windows-x64")
            .join("bin");
        std::fs::create_dir_all(&target_dir).unwrap();
        let target = target_dir.join("opencode.exe");
        std::fs::write(&target, "").unwrap();
        std::fs::write(
            bin.join("opencode.cmd"),
            r#"@ECHO off
"%dp0%\node.exe" "%dp0%\node_modules\opencode-ai\bin\opencode" %*
"#,
        )
        .unwrap();

        let old_path = std::env::var_os("PATH");
        let old_pathext = std::env::var_os("PATHEXT");
        std::env::set_var("PATH", &bin);
        std::env::set_var("PATHEXT", ".CMD;.EXE");

        let resolved = resolve_spawn_program("opencode");

        restore_env("PATH", old_path);
        restore_env("PATHEXT", old_pathext);

        assert_eq!(Path::new(&resolved), target);
    }

    #[test]
    #[cfg(windows)]
    fn resolve_spawn_program_unwraps_opencode_1151_native_shim() {
        let _guard = env_lock().lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("bin");
        let target_dir = bin.join("node_modules").join("opencode-ai").join("bin");
        std::fs::create_dir_all(&target_dir).unwrap();
        let target = target_dir.join("opencode.exe");
        std::fs::write(&target, "").unwrap();
        std::fs::write(
            bin.join("opencode.cmd"),
            r#"@ECHO off
SET dp0=%~dp0
"%dp0%\node_modules\opencode-ai\bin\opencode.exe" %*
"#,
        )
        .unwrap();

        let old_path = std::env::var_os("PATH");
        let old_pathext = std::env::var_os("PATHEXT");
        std::env::set_var("PATH", &bin);
        std::env::set_var("PATHEXT", ".CMD;.EXE");

        let resolved = resolve_spawn_program("opencode");

        restore_env("PATH", old_path);
        restore_env("PATHEXT", old_pathext);

        assert_eq!(Path::new(&resolved), target);
    }

    #[test]
    #[cfg(windows)]
    fn resolve_spawn_program_unwraps_explicit_cmd_path() {
        let _guard = env_lock().lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("tool.exe");
        let shim = temp.path().join("tool.cmd");
        std::fs::write(&target, "").unwrap();
        std::fs::write(&shim, format!(r#""{}" %*"#, target.display())).unwrap();

        let resolved = resolve_spawn_program(&shim.to_string_lossy());

        assert_eq!(Path::new(&resolved), target);
    }

    #[test]
    #[cfg(windows)]
    fn resolve_spawn_program_prefers_cmd_over_extensionless_npm_script() {
        let _guard = env_lock().lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("bin");
        std::fs::create_dir_all(&bin).unwrap();
        std::fs::write(
            bin.join("pi"),
            r#"#!/bin/sh
basedir=$(dirname "$(echo "$0" | sed -e 's,\\,/,g')")
exec "$basedir/node" "$basedir/node_modules/@earendil-works/pi-coding-agent/dist/cli.js" "$@"
"#,
        )
        .unwrap();
        std::fs::write(
            bin.join("pi.cmd"),
            r#"@ECHO off
"%dp0%\node.exe" "%dp0%\node_modules\@earendil-works\pi-coding-agent\dist\cli.js" %*
"#,
        )
        .unwrap();

        let old_path = std::env::var_os("PATH");
        let old_pathext = std::env::var_os("PATHEXT");
        std::env::set_var("PATH", &bin);
        std::env::set_var("PATHEXT", ".COM;.EXE;.BAT;.CMD;.PS1");

        let resolved = resolve_spawn_program("pi");

        restore_env("PATH", old_path);
        restore_env("PATHEXT", old_pathext);

        assert_eq!(Path::new(&resolved), bin.join("pi.cmd"));
    }

    #[test]
    #[cfg(windows)]
    fn resolve_pi_spawn_command_uses_node_cli_for_npm_shim() {
        let _guard = env_lock().lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("bin");
        let cli_dir = bin
            .join("node_modules")
            .join("@earendil-works")
            .join("pi-coding-agent")
            .join("dist");
        std::fs::create_dir_all(&cli_dir).unwrap();
        std::fs::write(bin.join("pi"), "#!/bin/sh\n").unwrap();
        std::fs::write(bin.join("pi.cmd"), r"@ECHO off").unwrap();
        std::fs::write(bin.join("node.exe"), "").unwrap();
        std::fs::write(cli_dir.join("cli.js"), "").unwrap();

        let old_path = std::env::var_os("PATH");
        let old_pathext = std::env::var_os("PATHEXT");
        std::env::set_var("PATH", &bin);
        std::env::set_var("PATHEXT", ".COM;.EXE;.BAT;.CMD;.PS1");

        let (program, args) = resolve_pi_spawn_command("pi");

        restore_env("PATH", old_path);
        restore_env("PATHEXT", old_pathext);

        assert_eq!(Path::new(&program), bin.join("node.exe"));
        assert_eq!(
            args,
            vec![cli_dir.join("cli.js").to_string_lossy().to_string()]
        );
    }

    #[test]
    #[cfg(windows)]
    fn resolve_codex_spawn_command_uses_node_cli_for_npm_shim() {
        let _guard = env_lock().lock().unwrap();
        let temp = tempfile::tempdir().unwrap();
        let bin = temp.path().join("bin");
        let cli_dir = bin
            .join("node_modules")
            .join("@openai")
            .join("codex")
            .join("bin");
        std::fs::create_dir_all(&cli_dir).unwrap();
        std::fs::write(bin.join("codex"), "#!/bin/sh\n").unwrap();
        std::fs::write(bin.join("codex.cmd"), r"@ECHO off").unwrap();
        std::fs::write(bin.join("node.exe"), "").unwrap();
        std::fs::write(cli_dir.join("codex.js"), "").unwrap();

        let old_path = std::env::var_os("PATH");
        let old_pathext = std::env::var_os("PATHEXT");
        std::env::set_var("PATH", &bin);
        std::env::set_var("PATHEXT", ".COM;.EXE;.BAT;.CMD;.PS1");

        let (program, args) = resolve_codex_spawn_command("codex");

        restore_env("PATH", old_path);
        restore_env("PATHEXT", old_pathext);

        assert_eq!(Path::new(&program), bin.join("node.exe"));
        assert_eq!(
            args,
            vec![cli_dir.join("codex.js").to_string_lossy().to_string()]
        );
    }

    #[cfg(windows)]
    fn restore_env(key: &str, value: Option<std::ffi::OsString>) {
        if let Some(value) = value {
            std::env::set_var(key, value);
        } else {
            std::env::remove_var(key);
        }
    }
}
