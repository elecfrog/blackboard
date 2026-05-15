//! 向后兼容的自动升级层 — 将旧格式 graph 升级为新的 Pin 格式。

use super::pins::default_pins_for;
use super::types::TaskGraphDefinition;

/// 将旧格式的 TaskGraphDefinition 升级为新格式：
/// 1. 为没有 pins 的 node 自动填充默认 pins
/// 2. 为没有 from_pin/to_pin 的 edge 根据 source_handle/target_handle 自动映射
pub fn upgrade_graph(graph: &mut TaskGraphDefinition) {
    upgrade_nodes(graph);
    upgrade_edges(graph);
}

/// 为没有 pins 字段的 node 自动填充默认 pins。
fn upgrade_nodes(graph: &mut TaskGraphDefinition) {
    for node in &mut graph.nodes {
        if node.pins.is_empty() {
            node.pins = default_pins_for(node.node_type, &node.config);
        }
    }
}

/// 为没有 from_pin/to_pin 的 edge 根据 source_handle/target_handle 自动映射。
fn upgrade_edges(graph: &mut TaskGraphDefinition) {
    for edge in &mut graph.edges {
        // 升级 from_pin
        if edge.from_pin.is_none() {
            edge.from_pin = Some(map_source_handle_to_from_pin(edge.source_handle.as_deref()));
        }
        // 升级 to_pin
        if edge.to_pin.is_none() {
            edge.to_pin = Some(map_target_handle_to_to_pin(edge.target_handle.as_deref()));
        }
    }
}

/// 将旧的 source_handle 映射为新的 from_pin ID。
///
/// 映射规则：
/// - None / "pin:xxx" → "exec_out"
/// - "body" → "body"
/// - "exit" → "exit"
/// - "rule:xxx" → "rule:xxx"
/// - 其他 → 原值
fn map_source_handle_to_from_pin(source_handle: Option<&str>) -> String {
    match source_handle {
        None => "exec_out".to_string(),
        Some(handle) => {
            if handle.starts_with("pin:") {
                // 旧的四向自由 pin 格式，映射为默认 exec_out
                "exec_out".to_string()
            } else {
                // "body", "exit", "rule:xxx" 等直接保留
                handle.to_string()
            }
        }
    }
}

/// 将旧的 target_handle 映射为新的 to_pin ID。
///
/// 映射规则：
/// - None / "pin:xxx" → "exec_in"
/// - "return" → "return"（legacy Loop return target，解释器会作为隐藏 frame return 兼容）
/// - 其他 → 原值
fn map_target_handle_to_to_pin(target_handle: Option<&str>) -> String {
    match target_handle {
        None => "exec_in".to_string(),
        Some(handle) => {
            if handle.starts_with("pin:") {
                "exec_in".to_string()
            } else {
                // "return" 等直接保留
                handle.to_string()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task_graph::definition::types::*;

    fn make_edge(source_handle: Option<&str>, target_handle: Option<&str>) -> TaskGraphEdge {
        TaskGraphEdge {
            id: "e1".into(),
            from: "a".into(),
            to: "b".into(),
            kind: EdgeKind::Exec,
            label: None,
            from_pin: None,
            to_pin: None,
            source_handle: source_handle.map(|s| s.to_string()),
            target_handle: target_handle.map(|s| s.to_string()),
        }
    }

    #[test]
    fn test_upgrade_edge_no_handles() {
        let mut graph = TaskGraphDefinition {
            schema_version: 1,
            id: "test".into(),
            scope: TaskGraphScope::Project,
            title: "Test".into(),
            description: None,
            version: 1,
            readonly: false,
            origin: None,
            metadata: None,
            inputs: None,
            nodes: vec![],
            edges: vec![make_edge(None, None)],
            layout: None,
        };
        upgrade_graph(&mut graph);
        assert_eq!(graph.edges[0].from_pin.as_deref(), Some("exec_out"));
        assert_eq!(graph.edges[0].to_pin.as_deref(), Some("exec_in"));
    }

    #[test]
    fn test_upgrade_edge_body_handle() {
        let mut graph = TaskGraphDefinition {
            schema_version: 1,
            id: "test".into(),
            scope: TaskGraphScope::Project,
            title: "Test".into(),
            description: None,
            version: 1,
            readonly: false,
            origin: None,
            metadata: None,
            inputs: None,
            nodes: vec![],
            edges: vec![make_edge(Some("body"), None)],
            layout: None,
        };
        upgrade_graph(&mut graph);
        assert_eq!(graph.edges[0].from_pin.as_deref(), Some("body"));
    }

    #[test]
    fn test_upgrade_edge_rule_handle() {
        let mut graph = TaskGraphDefinition {
            schema_version: 1,
            id: "test".into(),
            scope: TaskGraphScope::Project,
            title: "Test".into(),
            description: None,
            version: 1,
            readonly: false,
            origin: None,
            metadata: None,
            inputs: None,
            nodes: vec![],
            edges: vec![make_edge(Some("rule:yes"), None)],
            layout: None,
        };
        upgrade_graph(&mut graph);
        assert_eq!(graph.edges[0].from_pin.as_deref(), Some("rule:yes"));
    }

    #[test]
    fn test_upgrade_edge_pin_prefix_handle() {
        let mut graph = TaskGraphDefinition {
            schema_version: 1,
            id: "test".into(),
            scope: TaskGraphScope::Project,
            title: "Test".into(),
            description: None,
            version: 1,
            readonly: false,
            origin: None,
            metadata: None,
            inputs: None,
            nodes: vec![],
            edges: vec![make_edge(Some("pin:right"), Some("pin:left"))],
            layout: None,
        };
        upgrade_graph(&mut graph);
        assert_eq!(graph.edges[0].from_pin.as_deref(), Some("exec_out"));
        assert_eq!(graph.edges[0].to_pin.as_deref(), Some("exec_in"));
    }

    #[test]
    fn test_upgrade_edge_return_target() {
        let mut graph = TaskGraphDefinition {
            schema_version: 1,
            id: "test".into(),
            scope: TaskGraphScope::Project,
            title: "Test".into(),
            description: None,
            version: 1,
            readonly: false,
            origin: None,
            metadata: None,
            inputs: None,
            nodes: vec![],
            edges: vec![make_edge(None, Some("return"))],
            layout: None,
        };
        upgrade_graph(&mut graph);
        assert_eq!(graph.edges[0].to_pin.as_deref(), Some("return"));
    }

    #[test]
    fn test_upgrade_nodes_fills_pins() {
        let mut graph = TaskGraphDefinition {
            schema_version: 1,
            id: "test".into(),
            scope: TaskGraphScope::Project,
            title: "Test".into(),
            description: None,
            version: 1,
            readonly: false,
            origin: None,
            metadata: None,
            inputs: None,
            nodes: vec![TaskGraphNode {
                id: "start".into(),
                node_type: NodeType::Start,
                label: "Start".into(),
                description: None,
                position: None,
                config: serde_json::json!({}),
                pins: vec![],
            }],
            edges: vec![],
            layout: None,
        };
        upgrade_graph(&mut graph);
        assert!(!graph.nodes[0].pins.is_empty());
        assert_eq!(graph.nodes[0].pins[0].id, "exec_out");
    }

    #[test]
    fn test_upgrade_preserves_existing_pins() {
        let custom_pin = NodePin {
            id: "custom".into(),
            label: "Custom".into(),
            direction: PinDirection::Out,
            category: PinCategory::Exec,
            value_type: None,
            required: false,
        };
        let mut graph = TaskGraphDefinition {
            schema_version: 1,
            id: "test".into(),
            scope: TaskGraphScope::Project,
            title: "Test".into(),
            description: None,
            version: 1,
            readonly: false,
            origin: None,
            metadata: None,
            inputs: None,
            nodes: vec![TaskGraphNode {
                id: "start".into(),
                node_type: NodeType::Start,
                label: "Start".into(),
                description: None,
                position: None,
                config: serde_json::json!({}),
                pins: vec![custom_pin.clone()],
            }],
            edges: vec![],
            layout: None,
        };
        upgrade_graph(&mut graph);
        // 已有 pins 的节点不应被覆盖
        assert_eq!(graph.nodes[0].pins.len(), 1);
        assert_eq!(graph.nodes[0].pins[0].id, "custom");
    }
}
