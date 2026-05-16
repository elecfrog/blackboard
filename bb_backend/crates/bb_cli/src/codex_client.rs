#![allow(dead_code)]

//! Codex app-server JSON-RPC 2.0 client for daemon dispatch.
//!
//! Based on Multica's `server/pkg/agent/codex.go` implementation.
//! Protocol: `codex app-server --listen stdio://`
//! Sequence: initialize → initialized(notify) → thread/start → turn/start → [notifications] → turn/completed

use anyhow::{Context, Result};
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::sync::atomic::{AtomicI64, Ordering};

static REQUEST_ID: AtomicI64 = AtomicI64::new(1);

pub struct CodexAppServer {
    child: Child,
    reader: BufReader<ChildStdout>,
    pending_notifications: Vec<Value>,
}

impl CodexAppServer {
    /// Spawn `codex app-server --listen stdio://` and perform the initialize handshake.
    pub fn spawn(codex_path: &str, cwd: &Path, model: Option<&str>) -> Result<Self> {
        let codex_program = bb_core::platform::resolve_spawn_program(codex_path);
        let mut cmd = Command::new(&codex_program);
        cmd.arg("app-server")
            .arg("--listen")
            .arg("stdio://")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit());

        if let Some(m) = model {
            cmd.arg("-c").arg(format!("model=\"{m}\""));
        }

        cmd.current_dir(cwd);

        let mut child = cmd.spawn().context("spawn codex app-server")?;
        let stdout = child.stdout.take().context("take app-server stdout")?;
        let reader = BufReader::new(stdout);

        let mut server = Self {
            child,
            reader,
            pending_notifications: Vec::new(),
        };

        // 1. Send initialize request (matches Multica's flow)
        let _init_resp = server.request(
            "initialize",
            json!({
                "clientInfo": {
                    "name": "bb-daemon",
                    "title": "Blackboard Daemon",
                    "version": env!("CARGO_PKG_VERSION")
                },
                "capabilities": {
                    "experimentalApi": true
                }
            }),
        )?;

        // 2. Send "initialized" notification (CRITICAL: Multica does this at codex.go:196)
        server.notify("initialized")?;

        eprintln!("bb-daemon: codex app-server initialized");
        Ok(server)
    }

    /// Start a thread and run a single turn. Blocks until turn completes.
    /// Returns the agent's accumulated text output.
    pub fn run_turn(
        &mut self,
        prompt: &str,
        cwd: &Path,
        developer_instructions: Option<&str>,
    ) -> Result<String> {
        // thread/start — matches Multica's startOrResumeThread (codex.go:361-375)
        let thread_result = self.request(
            "thread/start",
            json!({
                "cwd": cwd.display().to_string(),
                "approvalPolicy": "never",
                "developerInstructions": developer_instructions,
                "persistExtendedHistory": true
            }),
        )?;

        // Extract threadId from response.thread.id
        let thread_id = thread_result
            .pointer("/thread/id")
            .and_then(|v| v.as_str())
            .context("thread/start response missing thread.id")?
            .to_string();

        eprintln!("bb-daemon: codex thread started: {thread_id}");

        // turn/start — matches Multica's codex.go:217-222
        let _turn_resp = self.request(
            "turn/start",
            json!({
                "threadId": thread_id,
                "input": [{"type": "text", "text": prompt}]
            }),
        )?;

        eprintln!("bb-daemon: codex turn started, waiting for completion...");

        // Read and process messages until turn completes
        let mut output = String::new();
        let mut turn_started = false;
        loop {
            let msg = self.read_message()?;
            let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");

            // Handle server requests (approval requests) — auto-approve everything
            // Matches Multica's handleServerRequest (codex.go:622-638)
            if self.handle_server_request(&msg)? {
                continue;
            }

            // Handle notifications
            match method {
                // Legacy notification format
                "codex/event" => {
                    if let Some(msg_data) = msg.pointer("/params/msg") {
                        let msg_type = msg_data.get("type").and_then(|v| v.as_str()).unwrap_or("");
                        match msg_type {
                            "task_started" => {
                                turn_started = true;
                            }
                            "agent_message" => {
                                if let Some(text) = msg_data.get("message").and_then(|v| v.as_str())
                                {
                                    eprint!("{text}");
                                    output.push_str(text);
                                }
                            }
                            "task_complete" => {
                                break;
                            }
                            "turn_aborted" => {
                                anyhow::bail!("codex turn was aborted");
                            }
                            _ => {}
                        }
                    }
                }
                // Raw v2 notification format
                "turn/completed" => {
                    let status = msg
                        .pointer("/params/turn/status")
                        .and_then(|v| v.as_str())
                        .unwrap_or("completed");
                    if status == "failed" {
                        let err = msg
                            .pointer("/params/turn/error/message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown error");
                        anyhow::bail!("codex turn failed: {err}");
                    }
                    break;
                }
                "turn/started" => {
                    turn_started = true;
                }
                "item/completed" => {
                    // Extract text from agentMessage items
                    let item_type = msg
                        .pointer("/params/item/type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if item_type == "agentMessage" {
                        if let Some(text) =
                            msg.pointer("/params/item/text").and_then(|v| v.as_str())
                        {
                            if !text.is_empty() {
                                output = text.to_string();
                            }
                        }
                    }
                }
                "item/agentMessage/delta" => {
                    if let Some(delta) = msg.pointer("/params/delta").and_then(|v| v.as_str()) {
                        eprint!("{delta}");
                    }
                }
                "thread/status/changed" => {
                    let status_type = msg
                        .pointer("/params/status/type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("");
                    if status_type == "idle" && turn_started {
                        // Thread went idle — turn is done
                        break;
                    }
                }
                "error" => {
                    let will_retry = msg
                        .pointer("/params/willRetry")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false);
                    if !will_retry {
                        let err_msg = msg
                            .pointer("/params/error/message")
                            .or_else(|| msg.pointer("/params/message"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown error");
                        anyhow::bail!("codex error: {err_msg}");
                    }
                }
                _ => {}
            }
        }

        eprintln!("\nbb-daemon: codex turn completed");
        Ok(output)
    }

    /// Send a JSON-RPC request and wait for the matching response.
    fn request(&mut self, method: &str, params: Value) -> Result<Value> {
        let id = REQUEST_ID.fetch_add(1, Ordering::SeqCst);
        self.write_message(json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params
        }))?;

        // Read messages until we get a response with matching id
        loop {
            let msg = self.read_raw_message()?;

            // Check if this is a response with our id
            if let Some(resp_id) = msg.get("id").and_then(|v| v.as_i64()) {
                if resp_id == id {
                    if let Some(err) = msg.get("error") {
                        let code = err.get("code").and_then(|v| v.as_i64()).unwrap_or(0);
                        let message = err
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown");
                        anyhow::bail!("codex {} failed: {} (code={})", method, message, code);
                    }
                    return Ok(msg.get("result").cloned().unwrap_or(Value::Null));
                }
            }

            if self.handle_server_request(&msg)? {
                continue;
            }

            // It's a notification or server request — buffer for later
            self.pending_notifications.push(msg);
        }
    }

    /// Send a JSON-RPC notification (no id, no response expected).
    fn notify(&mut self, method: &str) -> Result<()> {
        self.write_message(json!({
            "jsonrpc": "2.0",
            "method": method
        }))
    }

    /// Send a JSON-RPC response to a server request.
    fn respond(&mut self, id: Value, result: Value) -> Result<()> {
        self.write_message(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": result
        }))
    }

    /// Handle app-server initiated requests, such as approval prompts.
    fn handle_server_request(&mut self, msg: &Value) -> Result<bool> {
        if msg.get("id").is_none()
            || msg.get("method").is_none()
            || msg.get("result").is_some()
            || msg.get("error").is_some()
        {
            return Ok(false);
        }

        let id = msg.get("id").cloned().unwrap_or(Value::Null);
        let method = msg.get("method").and_then(|v| v.as_str()).unwrap_or("");
        match method {
            "item/commandExecution/requestApproval" | "execCommandApproval" => {
                self.respond(id, json!({"decision": "accept"}))?;
            }
            "item/fileChange/requestApproval" | "applyPatchApproval" => {
                self.respond(id, json!({"decision": "accept"}))?;
            }
            "permissionsRequest/approval" => {
                self.respond(id, json!({"decision": "accept"}))?;
            }
            _ => {
                self.respond(id, json!({}))?;
            }
        }
        Ok(true)
    }

    /// Write a JSON-RPC message to stdin.
    fn write_message(&mut self, msg: Value) -> Result<()> {
        let stdin = self
            .child
            .stdin
            .as_mut()
            .context("app-server stdin closed")?;
        let mut data = serde_json::to_string(&msg)?;
        data.push('\n');
        stdin
            .write_all(data.as_bytes())
            .context("write to codex stdin")?;
        stdin.flush()?;
        Ok(())
    }

    /// Read one parsed JSON-RPC message (checking buffered notifications first).
    fn read_message(&mut self) -> Result<Value> {
        // Drain buffered notifications first
        if !self.pending_notifications.is_empty() {
            return Ok(self.pending_notifications.remove(0));
        }

        self.read_raw_message()
    }

    /// Read one parsed JSON-RPC message directly from stdout.
    fn read_raw_message(&mut self) -> Result<Value> {
        loop {
            let mut line = String::new();
            let n = self
                .reader
                .read_line(&mut line)
                .context("read from codex stdout")?;
            if n == 0 {
                anyhow::bail!("codex app-server stdout closed unexpectedly");
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            match serde_json::from_str(trimmed) {
                Ok(v) => return Ok(v),
                Err(_) => continue, // Skip non-JSON lines
            }
        }
    }

    /// Gracefully shut down the app-server.
    pub fn shutdown(mut self) {
        drop(self.child.stdin.take());
        std::thread::sleep(std::time::Duration::from_millis(500));
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
