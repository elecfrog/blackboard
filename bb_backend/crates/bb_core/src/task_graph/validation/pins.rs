use std::collections::HashMap;

use super::super::types::*;

// ─── Pin connection validation ───────────────────────────────────────────────

/// 校验所有 edge 的 Pin 连接合法性：
/// - from_pin 必须存在于源节点 pins 中且方向为 out
/// - to_pin 必须存在于目标节点 pins 中且方向为 in
/// - 连接的两个 pin category 必须匹配
/// - required=true 的 InPin 必须有 edge 连入
pub(super) fn validate_pin_connections(
    def: &TaskGraphDefinition,
    node_map: &HashMap<&str, &TaskGraphNode>,
    incoming: &HashMap<&str, Vec<&TaskGraphEdge>>,
    errors: &mut Vec<TaskGraphValidationError>,
) {
    for (i, edge) in def.edges.iter().enumerate() {
        let from_node = node_map.get(edge.from.as_str());
        let to_node = node_map.get(edge.to.as_str());

        // 校验 from_pin
        if let (Some(from_pin_id), Some(from_node)) = (&edge.from_pin, from_node) {
            let from_pin = from_node.pins.iter().find(|p| &p.id == from_pin_id);
            match from_pin {
                None => {
                    errors.push(TaskGraphValidationError {
                        path: format!("edges[{}].from_pin", i),
                        code: "pin_not_found".to_string(),
                        message: format!(
                            "Edge '{}' from_pin '{}' not found on node '{}'",
                            edge.id, from_pin_id, edge.from
                        ),
                    });
                }
                Some(pin) => {
                    if pin.direction != PinDirection::Out {
                        errors.push(TaskGraphValidationError {
                            path: format!("edges[{}].from_pin", i),
                            code: "pin_direction_mismatch".to_string(),
                            message: format!(
                                "Edge '{}' from_pin '{}' on node '{}' is not an Out pin",
                                edge.id, from_pin_id, edge.from
                            ),
                        });
                    }
                }
            }
        }

        // 校验 to_pin
        if let (Some(to_pin_id), Some(to_node)) = (&edge.to_pin, to_node) {
            let legacy_loop_return = to_node.node_type == NodeType::Loop && to_pin_id == "return";
            if legacy_loop_return {
                continue;
            }
            let to_pin = to_node.pins.iter().find(|p| &p.id == to_pin_id);
            match to_pin {
                None => {
                    errors.push(TaskGraphValidationError {
                        path: format!("edges[{}].to_pin", i),
                        code: "pin_not_found".to_string(),
                        message: format!(
                            "Edge '{}' to_pin '{}' not found on node '{}'",
                            edge.id, to_pin_id, edge.to
                        ),
                    });
                }
                Some(pin) => {
                    if pin.direction != PinDirection::In {
                        errors.push(TaskGraphValidationError {
                            path: format!("edges[{}].to_pin", i),
                            code: "pin_direction_mismatch".to_string(),
                            message: format!(
                                "Edge '{}' to_pin '{}' on node '{}' is not an In pin",
                                edge.id, to_pin_id, edge.to
                            ),
                        });
                    }
                }
            }
        }

        // 校验 category 匹配
        if let (Some(from_pin_id), Some(to_pin_id), Some(from_node), Some(to_node)) =
            (&edge.from_pin, &edge.to_pin, from_node, to_node)
        {
            let from_pin = from_node.pins.iter().find(|p| &p.id == from_pin_id);
            let to_pin = to_node.pins.iter().find(|p| &p.id == to_pin_id);
            if let (Some(fp), Some(tp)) = (from_pin, to_pin) {
                if fp.category != tp.category {
                    errors.push(TaskGraphValidationError {
                        path: format!("edges[{}]", i),
                        code: "pin_category_mismatch".to_string(),
                        message: format!(
                            "Edge '{}' connects {:?} pin '{}' to {:?} pin '{}' — category mismatch",
                            edge.id, fp.category, from_pin_id, tp.category, to_pin_id
                        ),
                    });
                }
            }
        }
    }

    // 校验 required InPin 必须有 edge 连入
    for node in &def.nodes {
        for pin in &node.pins {
            if pin.required && pin.direction == PinDirection::In {
                let has_incoming = incoming
                    .get(node.id.as_str())
                    .map(|edges| edges.iter().any(|e| e.to_pin.as_deref() == Some(&pin.id)))
                    .unwrap_or(false);
                if !has_incoming {
                    // 对 Start 节点不报错（它没有 InPin）
                    // 对于 exec_in，如果节点有任何 incoming edge（即使没有 to_pin），也算通过
                    // 这是为了兼容升级过程中可能的中间状态
                    let has_any_incoming = incoming
                        .get(node.id.as_str())
                        .map(|edges| !edges.is_empty())
                        .unwrap_or(false);
                    if !has_any_incoming {
                        errors.push(TaskGraphValidationError {
                            path: format!("nodes[{}].pins[{}]", node.id, pin.id),
                            code: "required_pin_unconnected".to_string(),
                            message: format!(
                                "Node '{}' required InPin '{}' has no incoming edge",
                                node.id, pin.id
                            ),
                        });
                    }
                }
            }
        }
    }
}
