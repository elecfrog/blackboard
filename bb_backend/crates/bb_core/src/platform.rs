//! 平台相关工具函数（主机名、OS 版本等）。

use std::fs;
use std::path::PathBuf;

/// 获取当前机器主机名。
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
pub fn current_os_name() -> &'static str {
    match std::env::consts::OS {
        "windows" => "Windows",
        "macos" => "MacOS",
        "linux" => "Linux",
        other => other,
    }
}

/// 获取当前操作系统版本。
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
