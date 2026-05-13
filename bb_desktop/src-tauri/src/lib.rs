use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tauri::path::BaseDirectory;
use tauri::{Manager, RunEvent, WebviewUrl, WebviewWindowBuilder, WindowEvent};
use tauri_plugin_shell::process::{CommandChild, CommandEvent};
use tauri_plugin_shell::ShellExt;

const BACKEND_HOST: &str = "127.0.0.1";
const PREFERRED_BACKEND_PORT: u16 = 3001;
const BACKEND_PORT_FALLBACK_START: u16 = 3002;
const BACKEND_PORT_FALLBACK_END: u16 = 3099;

#[derive(Default)]
struct SidecarState {
    child: Mutex<Option<CommandChild>>,
    log_path: Mutex<Option<PathBuf>>,
}

#[derive(Debug, Clone)]
struct BackendEndpoint {
    addr: String,
    url: String,
    mcp_url: String,
    port: u16,
}

#[derive(Debug, Default, Deserialize, Serialize)]
struct DesktopStateFile {
    sidecar_addr: Option<String>,
    sidecar_url: Option<String>,
    mcp_url: Option<String>,
}

pub fn run() {
    let app = tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(SidecarState::default())
        .on_window_event(|window, event| {
            if window.label() == "main" && matches!(event, WindowEvent::CloseRequested { .. }) {
                stop_sidecar(window.app_handle());
            }
        })
        .setup(|app| {
            let handle = app.handle().clone();
            if let Err(err) = tauri::async_runtime::block_on(start_desktop(handle.clone())) {
                stop_sidecar(&handle);
                show_startup_error(&handle, &err)?;
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("failed to build Blackboard desktop app");

    app.run(|handle, event| {
        if matches!(event, RunEvent::ExitRequested { .. } | RunEvent::Exit) {
            stop_sidecar(handle);
        }
    });
}

async fn start_desktop(app: tauri::AppHandle) -> Result<(), String> {
    let global_root = global_bb_root()?;
    let desktop_root = global_root.join("runtime").join("desktop");
    let desktop_logs = desktop_root.join("logs");
    fs::create_dir_all(&desktop_logs).map_err(|err| {
        format!(
            "failed to create desktop logs directory {}: {err}",
            desktop_logs.display()
        )
    })?;

    let log_path = desktop_logs.join("sidecar.log");
    let desktop_state = desktop_root.join("state.json");
    set_log_path(&app, log_path.clone());
    append_log(&log_path, "Blackboard Desktop startup");
    let endpoint = select_backend_endpoint(&log_path)?;

    let home_template = app
        .path()
        .resolve("home-template", BaseDirectory::Resource)
        .map_err(|err| format!("failed to resolve bundled home template resource: {err}"))?;
    let web_root = app
        .path()
        .resolve("web", BaseDirectory::Resource)
        .map_err(|err| format!("failed to resolve bundled web resource: {err}"))?;

    if !home_template
        .join(".bb")
        .join("blackboard.json")
        .is_file()
    {
        return Err(format!(
            "bundled home template is missing .bb/blackboard.json under {}",
            home_template.display()
        ));
    }
    if home_template
        .join(".bb")
        .join("projects")
        .join("blackboard")
        .exists()
    {
        return Err(format!(
            "bundled home template must not include projects/blackboard under {}",
            home_template.display()
        ));
    }
    if !web_root.join("index.html").is_file() {
        return Err(format!(
            "bundled web frontend is missing index.html under {}",
            web_root.display()
        ));
    }

    if !valid_workspace_root(&global_root) {
        run_workspace_init(&app, &global_root, &home_template, &log_path).await?;
    }
    write_desktop_state(&desktop_state, &endpoint, &log_path)?;

    spawn_sidecar(&app, &global_root, &web_root, &endpoint, &log_path).await?;
    wait_for_healthz(&endpoint, &log_path)?;
    append_log(&log_path, "opening main window");
    open_main_window(&app, &endpoint)?;
    append_log(&log_path, "main window opened");
    Ok(())
}

async fn run_workspace_init(
    app: &tauri::AppHandle,
    workspace_root: &Path,
    template_root: &Path,
    log_path: &Path,
) -> Result<(), String> {
    append_log(
        log_path,
        &format!(
            "running workspace init: root={} template={}",
            workspace_root.display(),
            template_root.display()
        ),
    );

    let output = app
        .shell()
        .sidecar("bb")
        .map_err(|err| format!("failed to create bb init sidecar command: {err}"))?
        .args([
            "--root".to_string(),
            workspace_root.to_string_lossy().to_string(),
            "init".to_string(),
            "--seed".to_string(),
            template_root.to_string_lossy().to_string(),
        ])
        .output()
        .await
        .map_err(|err| format!("failed to run bb init sidecar command: {err}"))?;

    append_process_output(log_path, "init stdout", &output.stdout);
    append_process_output(log_path, "init stderr", &output.stderr);

    if !output.status.success() {
        return Err(format!(
            "bb init failed with status {:?}. See log: {}",
            output.status.code(),
            log_path.display()
        ));
    }

    Ok(())
}

async fn spawn_sidecar(
    app: &tauri::AppHandle,
    workspace_root: &Path,
    web_root: &Path,
    endpoint: &BackendEndpoint,
    log_path: &Path,
) -> Result<(), String> {
    let args = [
        "--root".to_string(),
        workspace_root.to_string_lossy().to_string(),
        "http".to_string(),
        "--addr".to_string(),
        endpoint.addr.clone(),
        "--static-dir".to_string(),
        web_root.to_string_lossy().to_string(),
    ];

    append_log(
        log_path,
        &format!("spawning bb sidecar with args: {args:?}"),
    );
    let (mut rx, child) = app
        .shell()
        .sidecar("bb")
        .map_err(|err| format!("failed to create bb http sidecar command: {err}"))?
        .args(args)
        .spawn()
        .map_err(|err| format!("failed to spawn bb http sidecar: {err}"))?;

    append_log(log_path, &format!("bb sidecar pid={}", child.pid()));
    store_sidecar_child(app, child);

    let log_path = log_path.to_path_buf();
    tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            match event {
                CommandEvent::Stdout(bytes) => append_process_output(&log_path, "stdout", &bytes),
                CommandEvent::Stderr(bytes) => append_process_output(&log_path, "stderr", &bytes),
                CommandEvent::Error(message) => {
                    append_log(&log_path, &format!("sidecar event error: {message}"));
                }
                CommandEvent::Terminated(payload) => {
                    append_log(&log_path, &format!("sidecar terminated: {payload:?}"));
                }
                _ => {}
            }
        }
    });

    Ok(())
}

fn open_main_window(app: &tauri::AppHandle, endpoint: &BackendEndpoint) -> Result<(), String> {
    let url = endpoint
        .url
        .parse()
        .map_err(|err| format!("invalid backend URL {}: {err}", endpoint.url))?;
    WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
        .title("Blackboard")
        .inner_size(1280.0, 820.0)
        .min_inner_size(960.0, 640.0)
        .build()
        .map_err(|err| format!("failed to create Blackboard window: {err}"))?;
    Ok(())
}

fn show_startup_error(
    app: &tauri::AppHandle,
    message: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let global_root = global_bb_root().unwrap_or_else(|_| std::env::temp_dir().join(".bb"));
    let desktop_root = global_root.join("runtime").join("desktop");
    let desktop_logs = desktop_root.join("logs");
    fs::create_dir_all(&desktop_logs)?;
    let html_path = desktop_root.join("startup-error.html");
    let log_path = current_log_path(app).unwrap_or_else(|| desktop_logs.join("sidecar.log"));
    fs::write(
        &html_path,
        format!(
            r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>Blackboard Startup Error</title>
  <style>
    :root {{ color-scheme: light dark; font-family: Inter, Segoe UI, system-ui, sans-serif; }}
    body {{ margin: 0; min-height: 100vh; display: grid; place-items: center; background: #f6f7f9; color: #172033; }}
    main {{ width: min(720px, calc(100vw - 48px)); }}
    h1 {{ font-size: 24px; margin: 0 0 12px; }}
    p {{ line-height: 1.55; }}
    pre {{ white-space: pre-wrap; padding: 14px; background: #fff; border: 1px solid #d8dde8; border-radius: 8px; }}
    @media (prefers-color-scheme: dark) {{
      body {{ background: #111827; color: #e5e7eb; }}
      pre {{ background: #0b1220; border-color: #263244; }}
    }}
  </style>
</head>
<body>
  <main>
    <h1>Blackboard could not start</h1>
    <p>The local sidecar server did not start. Details:</p>
    <pre>{}</pre>
    <p>Log file: <code>{}</code></p>
  </main>
</body>
</html>
"#,
            escape_html(message),
            escape_html(&log_path.display().to_string())
        ),
    )?;

    let url = tauri::Url::from_file_path(&html_path)
        .map_err(|_| format!("failed to convert {} to file URL", html_path.display()))?;
    WebviewWindowBuilder::new(app, "main", WebviewUrl::External(url))
        .title("Blackboard Startup Error")
        .inner_size(820.0, 560.0)
        .min_inner_size(640.0, 420.0)
        .build()?;
    Ok(())
}

fn wait_for_healthz(endpoint: &BackendEndpoint, log_path: &Path) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(15);
    while Instant::now() < deadline {
        if healthz_ok(&endpoint.addr) {
            append_log(log_path, &format!("healthz ok at {}", endpoint.url));
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(150));
    }

    Err(format!(
        "bb sidecar did not become healthy at {}healthz. See log: {}",
        endpoint.url,
        log_path.display()
    ))
}

fn healthz_ok(addr: &str) -> bool {
    let Ok(mut stream) = TcpStream::connect(addr) else {
        return false;
    };
    let _ = stream.set_read_timeout(Some(Duration::from_millis(500)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    if stream
        .write_all(b"GET /healthz HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        return false;
    }
    let mut response = [0_u8; 128];
    match stream.read(&mut response) {
        Ok(size) => String::from_utf8_lossy(&response[..size]).starts_with("HTTP/1.1 200"),
        Err(_) => false,
    }
}

fn select_backend_endpoint(log_path: &Path) -> Result<BackendEndpoint, String> {
    if port_available(PREFERRED_BACKEND_PORT) {
        let endpoint = backend_endpoint(PREFERRED_BACKEND_PORT);
        append_log(
            log_path,
            &format!("selected preferred sidecar port {}", endpoint.port),
        );
        return Ok(endpoint);
    }

    append_log(
        log_path,
        &format!(
            "preferred sidecar port {PREFERRED_BACKEND_PORT} is in use; scanning fallback range {BACKEND_PORT_FALLBACK_START}-{BACKEND_PORT_FALLBACK_END}"
        ),
    );
    for port in BACKEND_PORT_FALLBACK_START..=BACKEND_PORT_FALLBACK_END {
        if port_available(port) {
            let endpoint = backend_endpoint(port);
            append_log(
                log_path,
                &format!(
                    "selected fallback sidecar port {} because {}:{} is already in use",
                    endpoint.port, BACKEND_HOST, PREFERRED_BACKEND_PORT
                ),
            );
            return Ok(endpoint);
        }
    }

    Err(format!(
        "could not find a free Blackboard Desktop sidecar port in {BACKEND_HOST}:{PREFERRED_BACKEND_PORT} or fallback range {BACKEND_PORT_FALLBACK_START}-{BACKEND_PORT_FALLBACK_END}. Blackboard did not stop any existing process.\n\nLog: {}",
        log_path.display()
    ))
}

fn backend_endpoint(port: u16) -> BackendEndpoint {
    let addr = format!("{BACKEND_HOST}:{port}");
    let url = format!("http://{addr}/");
    let mcp_url = format!("http://{addr}/mcp");
    BackendEndpoint {
        addr,
        url,
        mcp_url,
        port,
    }
}

fn port_available(port: u16) -> bool {
    TcpListener::bind(SocketAddrV4::new(Ipv4Addr::LOCALHOST, port)).is_ok()
}

fn read_desktop_state(path: &Path) -> Option<DesktopStateFile> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_desktop_state(
    path: &Path,
    endpoint: &BackendEndpoint,
    log_path: &Path,
) -> Result<(), String> {
    let mut state = read_desktop_state(path).unwrap_or_default();
    state.sidecar_addr = Some(endpoint.addr.clone());
    state.sidecar_url = Some(endpoint.url.clone());
    state.mcp_url = Some(endpoint.mcp_url.clone());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create desktop state directory {}: {err}",
                parent.display()
            )
        })?;
    }
    let text = serde_json::to_string_pretty(&state)
        .map_err(|err| format!("failed to serialize desktop state: {err}"))?;
    fs::write(path, format!("{text}\n"))
        .map_err(|err| format!("failed to write desktop state {}: {err}", path.display()))?;
    append_log(
        log_path,
        &format!(
            "desktop state updated: sidecar_url={} mcp_url={}",
            endpoint.url, endpoint.mcp_url
        ),
    );
    Ok(())
}

fn valid_workspace_root(root: &Path) -> bool {
    if root.join("blackboard.json").is_file()
        && root.join("projects").join("__projects__.json").is_file()
    {
        return true;
    }

    root.join(".bb").join("blackboard.json").is_file()
        && root
            .join(".bb")
            .join("projects")
            .join("__projects__.json")
            .is_file()
}

fn global_bb_root() -> Result<PathBuf, String> {
    Ok(user_home_dir()?.join(".bb"))
}

fn user_home_dir() -> Result<PathBuf, String> {
    if cfg!(target_os = "windows") {
        if let Some(userprofile) = std::env::var_os("USERPROFILE") {
            return Ok(PathBuf::from(userprofile));
        }
        if let (Some(home_drive), Some(home_path)) =
            (std::env::var_os("HOMEDRIVE"), std::env::var_os("HOMEPATH"))
        {
            let mut home = PathBuf::from(home_drive);
            home.push(home_path);
            return Ok(home);
        }
        if let Some(home) = std::env::var_os("HOME") {
            return Ok(PathBuf::from(home));
        }
        return Err("USERPROFILE is not set; cannot resolve ~/.bb".to_string());
    }

    let home = std::env::var_os("HOME")
        .ok_or_else(|| "HOME is not set; cannot resolve ~/.bb".to_string())?;
    Ok(PathBuf::from(home))
}

fn store_sidecar_child(app: &tauri::AppHandle, child: CommandChild) {
    let state = app.state::<SidecarState>();
    *state.child.lock().expect("sidecar child lock poisoned") = Some(child);
}

fn stop_sidecar(app: &tauri::AppHandle) {
    let child = {
        let state = app.state::<SidecarState>();
        let child = state
            .child
            .lock()
            .expect("sidecar child lock poisoned")
            .take();
        child
    };

    if let Some(child) = child {
        if let Err(err) = child.kill() {
            if let Some(log_path) = current_log_path(app) {
                append_log(&log_path, &format!("failed to kill sidecar: {err}"));
            }
        }
    }
}

fn set_log_path(app: &tauri::AppHandle, log_path: PathBuf) {
    let state = app.state::<SidecarState>();
    *state.log_path.lock().expect("sidecar log lock poisoned") = Some(log_path);
}

fn current_log_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    app.state::<SidecarState>()
        .log_path
        .lock()
        .expect("sidecar log lock poisoned")
        .clone()
}

fn append_process_output(log_path: &Path, label: &str, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    let text = String::from_utf8_lossy(bytes);
    append_log(log_path, &format!("{label}: {text}"));
}

fn append_log(log_path: &Path, message: &str) {
    if let Some(parent) = log_path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(log_path) {
        let _ = writeln!(file, "{message}");
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
