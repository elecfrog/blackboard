use axum::extract::{Path, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use bb_core::task_graph;
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path as StdPath;
use std::time::Duration;

use super::super::AppState;
use super::current_workspace_root;
use super::dto::{TaskGraphApiError, TgRunDetailResponse};

/// GET /api/projects/{project}/task-graph-runs/{run_id}/events — stream run snapshots
pub async fn tg_run_events(
    State(state): State<AppState>,
    Path((project, run_id)): Path<(String, String)>,
) -> Result<Response, TaskGraphApiError> {
    let root = current_workspace_root(&state)?;
    task_graph::read_run_detail(&root, &project, &run_id)?;
    let run_dir = root
        .join("runtime")
        .join("task_graph_runs")
        .join(&project)
        .join(&run_id);

    let stream = async_stream::stream! {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut watcher: RecommendedWatcher = match notify::recommended_watcher(move |result| {
            let _ = tx.send(result);
        }) {
            Ok(watcher) => watcher,
            Err(err) => {
                yield Ok::<Event, std::convert::Infallible>(
                    Event::default().event("run_error").data(task_graph_run_error_payload(&err.to_string())),
                );
                return;
            }
        };
        if let Err(err) = watcher.watch(&run_dir, RecursiveMode::Recursive) {
            yield Ok::<Event, std::convert::Infallible>(
                Event::default().event("run_error").data(task_graph_run_error_payload(&err.to_string())),
            );
            return;
        }

        let (payload, terminal) = match task_graph_run_payload(&root, &project, &run_id) {
            Ok((payload, terminal)) => (payload, terminal),
            Err(message) => {
                yield Ok::<Event, std::convert::Infallible>(
                    Event::default().event("run_error").data(task_graph_run_error_payload(&message)),
                );
                return;
            }
        };
        let mut last_payload = payload.clone();
        yield Ok::<Event, std::convert::Infallible>(
            Event::default().event("run").data(payload),
        );
        if terminal {
            return;
        }

        while let Some(result) = rx.recv().await {
            if let Err(err) = result {
                yield Ok::<Event, std::convert::Infallible>(
                    Event::default().event("run_error").data(task_graph_run_error_payload(&err.to_string())),
                );
                return;
            }

            tokio::time::sleep(Duration::from_millis(20)).await;
            match task_graph_run_payload(&root, &project, &run_id) {
                Ok((payload, terminal)) => {
                    if payload != last_payload {
                        last_payload = payload.clone();
                        yield Ok::<Event, std::convert::Infallible>(
                            Event::default().event("run").data(payload),
                        );
                    }
                    if terminal {
                        return;
                    }
                }
                Err(message) => {
                    yield Ok::<Event, std::convert::Infallible>(
                        Event::default().event("run_error").data(task_graph_run_error_payload(&message)),
                    );
                    return;
                }
            }
        }
    };

    Ok(Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response())
}

fn task_graph_run_payload(
    root: &StdPath,
    project: &str,
    run_id: &str,
) -> Result<(String, bool), String> {
    let detail =
        task_graph::read_run_detail(root, project, run_id).map_err(|err| err.to_string())?;
    let terminal = matches!(
        detail.run.status,
        task_graph::RunStatus::Succeeded
            | task_graph::RunStatus::Failed
            | task_graph::RunStatus::Cancelled
    );
    let payload = serde_json::to_string(&TgRunDetailResponse { run: detail })
        .map_err(|err| err.to_string())?;
    Ok((payload, terminal))
}

fn task_graph_run_error_payload(message: &str) -> String {
    serde_json::json!({
        "error": { "message": message }
    })
    .to_string()
}
