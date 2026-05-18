use std::collections::{HashMap, HashSet};

use crate::task_graph::definition::types::{
    NodeType, TaskGraphDefinition, TaskGraphNode, TaskGraphValidationError,
};

// ─── Cycle detection ─────────────────────────────────────────────────────────

/// Rule 10: Any cycle must go through a Loop node.
///
/// Strategy: remove all loop nodes from the graph (and their incident edges).
/// If the remaining subgraph has a cycle, that cycle is illegal because it doesn't
/// pass through any loop node.
pub(super) fn validate_no_illegal_cycles(
    def: &TaskGraphDefinition,
    node_map: &HashMap<&str, &TaskGraphNode>,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    let loop_node_ids: HashSet<&str> = def
        .nodes
        .iter()
        .filter(|n| n.node_type == NodeType::Loop)
        .map(|n| n.id.as_str())
        .collect();

    // Build adjacency list excluding any edge that touches a loop node
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    for edge in &def.edges {
        // Skip edges involving loop nodes — cycles through them are allowed
        if loop_node_ids.contains(edge.from.as_str()) || loop_node_ids.contains(edge.to.as_str()) {
            continue;
        }
        // Only include edges whose endpoints exist
        if node_map.contains_key(edge.from.as_str()) && node_map.contains_key(edge.to.as_str()) {
            adj.entry(edge.from.as_str())
                .or_default()
                .push(edge.to.as_str());
        }
    }

    // DFS cycle detection on the remaining subgraph
    #[derive(Clone, Copy, PartialEq)]
    enum Color {
        White,
        Gray,
        Black,
    }

    let mut color: HashMap<&str, Color> = def
        .nodes
        .iter()
        .filter(|n| !loop_node_ids.contains(n.id.as_str()))
        .map(|n| (n.id.as_str(), Color::White))
        .collect();

    fn dfs<'a>(
        node: &'a str,
        adj: &HashMap<&'a str, Vec<&'a str>>,
        color: &mut HashMap<&'a str, Color>,
        has_cycle: &mut bool,
    ) {
        color.insert(node, Color::Gray);
        if let Some(neighbors) = adj.get(node) {
            for &next in neighbors {
                match color.get(next) {
                    Some(Color::Gray) => {
                        *has_cycle = true;
                        return;
                    }
                    Some(Color::White) => {
                        dfs(next, adj, color, has_cycle);
                        if *has_cycle {
                            return;
                        }
                    }
                    _ => {}
                }
            }
        }
        color.insert(node, Color::Black);
    }

    let mut has_cycle = false;
    let non_loop_nodes: Vec<&str> = def
        .nodes
        .iter()
        .filter(|n| !loop_node_ids.contains(n.id.as_str()))
        .map(|n| n.id.as_str())
        .collect();

    for node in &non_loop_nodes {
        if color[node] == Color::White {
            dfs(node, &adj, &mut color, &mut has_cycle);
            if has_cycle {
                break;
            }
        }
    }

    if has_cycle {
        errors.push(TaskGraphValidationError {
            path: "edges".to_string(),
            code: "invalid_cycle".to_string(),
            message: "Graph contains a cycle that does not pass through a loop node".to_string(),
        });
    }
}
