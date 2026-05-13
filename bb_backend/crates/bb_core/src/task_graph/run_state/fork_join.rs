use std::collections::{HashMap, HashSet};
use std::path::Path;

use chrono::Utc;

use super::super::types::{EdgeKind, TaskGraphEdge, TaskGraphError};
use super::model::TaskGraphRun;
use super::{read_json, run_dir, run_json_path, write_json};

/// 静态分析 edge 列表，找出所有 exec 入度 > 1 的节点作为 join 点。
/// 仅考虑 Exec 类型的边。
pub fn detect_join_nodes(edges: &[TaskGraphEdge]) -> HashSet<String> {
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    for edge in edges {
        if edge.kind == EdgeKind::Exec {
            *in_degree.entry(edge.to.as_str()).or_insert(0) += 1;
        }
    }
    in_degree
        .into_iter()
        .filter(|(_, count)| *count > 1)
        .map(|(node_id, _)| node_id.to_string())
        .collect()
}

/// 记录某条分支到达 join 点。
/// 将 `from_node_id`（完成的上游节点）添加到 join 点的已完成分支列表中。
pub fn record_branch_completion(
    workspace_root: &Path,
    project: &str,
    run_id: &str,
    join_node_id: &str,
    from_node_id: &str,
) -> Result<(), TaskGraphError> {
    let dir = run_dir(workspace_root, project, run_id);
    let run_file = run_json_path(&dir);

    let mut run: TaskGraphRun = read_json(&run_file)?;
    run.context
        .completed_branches
        .entry(join_node_id.to_string())
        .or_default()
        .push(from_node_id.to_string());
    run.updated_at = Utc::now().to_rfc3339();
    write_json(&run_file, &run)?;
    Ok(())
}

/// 判断 join 点的所有 exec 入边是否都已完成。
/// `edge_map_incoming` 是以目标节点 ID 为 key 的入边映射。
pub fn is_join_ready(run: &TaskGraphRun, join_node_id: &str, expected_count: usize) -> bool {
    match run.context.completed_branches.get(join_node_id) {
        Some(completed) => completed.len() >= expected_count,
        None => expected_count == 0,
    }
}

/// 计算某个节点的 exec 入边数量。
pub fn count_exec_in_edges(edges: &[TaskGraphEdge], node_id: &str) -> usize {
    edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Exec && e.to == node_id)
        .count()
}
