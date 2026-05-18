use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Condvar, Mutex, OnceLock};
use std::time::Instant;

use serde::Deserialize;

const RUNNER_CONFIG_PATH: &str = "config/task_graph_runner.toml";

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
struct RuntimeConcurrencyConfig {
    runtime_max_concurrency: BTreeMap<String, usize>,
    opencode_max_concurrent: Option<usize>,
    codex_max_concurrent: Option<usize>,
    codebuddy_max_concurrent: Option<usize>,
}

#[derive(Debug)]
struct RuntimeGate {
    state: Mutex<RuntimeGateState>,
    available: Condvar,
}

#[derive(Debug, Default)]
struct RuntimeGateState {
    limit: usize,
    in_flight: usize,
}

#[derive(Debug)]
pub struct RuntimePermit {
    gate: Arc<RuntimeGate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RuntimePermitInfo {
    pub limit: usize,
    pub waited_ms: u64,
}

static GATES: OnceLock<Mutex<HashMap<String, Arc<RuntimeGate>>>> = OnceLock::new();

pub fn configured_limit(workspace_root: &Path, runtime: &str) -> Option<usize> {
    read_runtime_concurrency_config(workspace_root)
        .limit_for(runtime)
        .filter(|limit| *limit > 0)
}

pub fn acquire(
    workspace_root: &Path,
    runtime: &str,
    limit: usize,
) -> (RuntimePermit, RuntimePermitInfo) {
    let started = Instant::now();
    let gate = gate_for(workspace_root, runtime);
    let mut state = gate
        .state
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    state.limit = limit.max(1);
    while state.in_flight >= state.limit {
        state = gate
            .available
            .wait(state)
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.limit = limit.max(1);
    }
    state.in_flight += 1;
    drop(state);

    (
        RuntimePermit { gate },
        RuntimePermitInfo {
            limit: limit.max(1),
            waited_ms: started.elapsed().as_millis() as u64,
        },
    )
}

impl Drop for RuntimePermit {
    fn drop(&mut self) {
        let mut state = self
            .gate
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.in_flight = state.in_flight.saturating_sub(1);
        drop(state);
        self.gate.available.notify_one();
    }
}

impl RuntimeConcurrencyConfig {
    fn limit_for(&self, runtime: &str) -> Option<usize> {
        self.runtime_max_concurrency
            .get(runtime)
            .copied()
            .or(match runtime {
                "opencode" => self.opencode_max_concurrent,
                "codex" => self.codex_max_concurrent,
                "codebuddy" => self.codebuddy_max_concurrent,
                _ => None,
            })
    }
}

fn gate_for(workspace_root: &Path, runtime: &str) -> Arc<RuntimeGate> {
    let key = format!(
        "{}::{}",
        workspace_root.display().to_string().replace('\\', "/"),
        runtime
    );
    let gates = GATES.get_or_init(|| Mutex::new(HashMap::new()));
    let mut gates = gates
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    gates
        .entry(key)
        .or_insert_with(|| {
            Arc::new(RuntimeGate {
                state: Mutex::new(RuntimeGateState::default()),
                available: Condvar::new(),
            })
        })
        .clone()
}

fn read_runtime_concurrency_config(workspace_root: &Path) -> RuntimeConcurrencyConfig {
    let path = workspace_root.join(RUNNER_CONFIG_PATH);
    let Ok(content) = std::fs::read_to_string(&path) else {
        return RuntimeConcurrencyConfig::default();
    };
    toml::from_str(&content).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn reads_runtime_limit_from_runner_config() {
        let temp = tempfile::tempdir().unwrap();
        let config_dir = temp.path().join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("task_graph_runner.toml"),
            r"
[runtime_max_concurrency]
opencode = 1
codex = 2
",
        )
        .unwrap();

        assert_eq!(configured_limit(temp.path(), "opencode"), Some(1));
        assert_eq!(configured_limit(temp.path(), "codex"), Some(2));
        assert_eq!(configured_limit(temp.path(), "codebuddy"), None);
    }

    #[test]
    fn runtime_gate_blocks_until_permit_is_released() {
        let temp = tempfile::tempdir().unwrap();
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let root = temp.path().to_path_buf();
        let first = thread::spawn(move || {
            let (_permit, _) = acquire(&root, "opencode-test", 1);
            entered_tx.send(()).unwrap();
            release_rx.recv().unwrap();
        });
        entered_rx.recv().unwrap();

        let root = temp.path().to_path_buf();
        let second = thread::spawn(move || {
            let (_permit, info) = acquire(&root, "opencode-test", 1);
            info.waited_ms
        });
        thread::sleep(Duration::from_millis(50));
        assert!(!second.is_finished());
        release_tx.send(()).unwrap();

        first.join().unwrap();
        assert!(second.join().unwrap() >= 40);
    }
}
