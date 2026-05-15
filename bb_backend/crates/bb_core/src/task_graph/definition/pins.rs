//! 默认 Pin 生成逻辑 — 根据节点类型和配置生成标准 Pin 集合。

use super::types::{BranchConfig, NodePin, NodeType, PinCategory, PinDirection, PinValueType};

/// 根据节点类型和配置生成默认的 Pin 集合。
///
/// 当节点的 `pins` 字段为空时，调用此函数自动填充。
pub fn default_pins_for(node_type: NodeType, config: &serde_json::Value) -> Vec<NodePin> {
    match node_type {
        NodeType::Start => pins_start(),
        NodeType::End => pins_end(),
        NodeType::Llm => pins_llm(),
        NodeType::Shell => pins_shell(),
        NodeType::Branch => pins_branch(config),
        NodeType::Loop => pins_loop(),
        NodeType::HumanGate => pins_human_gate(),
        NodeType::InputVar => pins_input_var(),
        NodeType::SubGraph => pins_sub_graph(),
    }
}

// ─── Start ───────────────────────────────────────────────────────────────────

fn pins_start() -> Vec<NodePin> {
    vec![NodePin {
        id: "exec_out".into(),
        label: "Out".into(),
        direction: PinDirection::Out,
        category: PinCategory::Exec,
        value_type: None,
        required: false,
    }]
}

// ─── End ─────────────────────────────────────────────────────────────────────

fn pins_end() -> Vec<NodePin> {
    vec![NodePin {
        id: "exec_in".into(),
        label: "In".into(),
        direction: PinDirection::In,
        category: PinCategory::Exec,
        value_type: None,
        required: true,
    }]
}

// ─── Llm ─────────────────────────────────────────────────────────────────────

fn pins_llm() -> Vec<NodePin> {
    vec![
        NodePin {
            id: "exec_in".into(),
            label: "In".into(),
            direction: PinDirection::In,
            category: PinCategory::Exec,
            value_type: None,
            required: true,
        },
        NodePin {
            id: "exec_out".into(),
            label: "Out".into(),
            direction: PinDirection::Out,
            category: PinCategory::Exec,
            value_type: None,
            required: false,
        },
        NodePin {
            id: "output".into(),
            label: "Output".into(),
            direction: PinDirection::Out,
            category: PinCategory::Data,
            value_type: Some(PinValueType::Json),
            required: false,
        },
    ]
}

// ─── Shell ──────────────────────────────────────────────────────────────────

fn pins_shell() -> Vec<NodePin> {
    vec![
        NodePin {
            id: "exec_in".into(),
            label: "In".into(),
            direction: PinDirection::In,
            category: PinCategory::Exec,
            value_type: None,
            required: true,
        },
        NodePin {
            id: "exec_out".into(),
            label: "Out".into(),
            direction: PinDirection::Out,
            category: PinCategory::Exec,
            value_type: None,
            required: false,
        },
        NodePin {
            id: "output".into(),
            label: "Output".into(),
            direction: PinDirection::Out,
            category: PinCategory::Data,
            value_type: Some(PinValueType::Json),
            required: false,
        },
    ]
}

// ─── Branch ──────────────────────────────────────────────────────────────────

fn pins_branch(config: &serde_json::Value) -> Vec<NodePin> {
    let mut pins = vec![NodePin {
        id: "exec_in".into(),
        label: "In".into(),
        direction: PinDirection::In,
        category: PinCategory::Exec,
        value_type: None,
        required: true,
    }];

    // 尝试从 config 中解析 BranchConfig 获取 rules
    if let Ok(branch_config) = serde_json::from_value::<BranchConfig>(config.clone()) {
        for rule in &branch_config.rules {
            pins.push(NodePin {
                id: format!("rule:{}", rule.id),
                label: rule.label.clone(),
                direction: PinDirection::Out,
                category: PinCategory::Exec,
                value_type: None,
                required: false,
            });
        }
    }

    pins
}

// ─── Loop ────────────────────────────────────────────────────────────────────

fn pins_loop() -> Vec<NodePin> {
    vec![
        NodePin {
            id: "exec_in".into(),
            label: "In".into(),
            direction: PinDirection::In,
            category: PinCategory::Exec,
            value_type: None,
            required: true,
        },
        NodePin {
            id: "body".into(),
            label: "Body".into(),
            direction: PinDirection::Out,
            category: PinCategory::Exec,
            value_type: None,
            required: false,
        },
        NodePin {
            id: "exit".into(),
            label: "Exit".into(),
            direction: PinDirection::Out,
            category: PinCategory::Exec,
            value_type: None,
            required: false,
        },
        NodePin {
            id: "index".into(),
            label: "Index".into(),
            direction: PinDirection::Out,
            category: PinCategory::Data,
            value_type: Some(PinValueType::Int),
            required: false,
        },
    ]
}

// ─── HumanGate ───────────────────────────────────────────────────────────────

fn pins_human_gate() -> Vec<NodePin> {
    vec![
        NodePin {
            id: "exec_in".into(),
            label: "In".into(),
            direction: PinDirection::In,
            category: PinCategory::Exec,
            value_type: None,
            required: true,
        },
        NodePin {
            id: "exec_out".into(),
            label: "Out".into(),
            direction: PinDirection::Out,
            category: PinCategory::Exec,
            value_type: None,
            required: false,
        },
        NodePin {
            id: "action".into(),
            label: "Action".into(),
            direction: PinDirection::Out,
            category: PinCategory::Data,
            value_type: Some(PinValueType::String),
            required: false,
        },
    ]
}

// ─── SubGraph ────────────────────────────────────────────────────────────────────

fn pins_sub_graph() -> Vec<NodePin> {
    vec![
        NodePin {
            id: "exec_in".into(),
            label: "In".into(),
            direction: PinDirection::In,
            category: PinCategory::Exec,
            value_type: None,
            required: true,
        },
        NodePin {
            id: "exec_out".into(),
            label: "Out".into(),
            direction: PinDirection::Out,
            category: PinCategory::Exec,
            value_type: None,
            required: false,
        },
        NodePin {
            id: "output".into(),
            label: "Output".into(),
            direction: PinDirection::Out,
            category: PinCategory::Data,
            value_type: Some(PinValueType::Json),
            required: false,
        },
    ]
}

// ─── InputVar ──────────────────────────────────────────────────────────────────────

fn pins_input_var() -> Vec<NodePin> {
    vec![
        NodePin {
            id: "exec_in".into(),
            label: "In".into(),
            direction: PinDirection::In,
            category: PinCategory::Exec,
            value_type: None,
            required: true,
        },
        NodePin {
            id: "exec_out".into(),
            label: "Out".into(),
            direction: PinDirection::Out,
            category: PinCategory::Exec,
            value_type: None,
            required: false,
        },
        NodePin {
            id: "value".into(),
            label: "Value".into(),
            direction: PinDirection::Out,
            category: PinCategory::Data,
            value_type: Some(PinValueType::Any),
            required: false,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_start_pins() {
        let pins = default_pins_for(
            NodeType::Start,
            &serde_json::Value::Object(Default::default()),
        );
        assert_eq!(pins.len(), 1);
        assert_eq!(pins[0].id, "exec_out");
        assert_eq!(pins[0].direction, PinDirection::Out);
        assert_eq!(pins[0].category, PinCategory::Exec);
    }

    #[test]
    fn test_end_pins() {
        let pins = default_pins_for(NodeType::End, &serde_json::json!({"result": "succeeded"}));
        assert_eq!(pins.len(), 1);
        assert_eq!(pins[0].id, "exec_in");
        assert_eq!(pins[0].direction, PinDirection::In);
        assert!(pins[0].required);
    }

    #[test]
    fn test_llm_pins() {
        let pins = default_pins_for(
            NodeType::Llm,
            &serde_json::Value::Object(Default::default()),
        );
        assert_eq!(pins.len(), 3);
        assert_eq!(pins[0].id, "exec_in");
        assert_eq!(pins[1].id, "exec_out");
        assert_eq!(pins[2].id, "output");
        assert_eq!(pins[2].category, PinCategory::Data);
        assert_eq!(pins[2].value_type, Some(PinValueType::Json));
    }

    #[test]
    fn test_shell_pins() {
        let pins = default_pins_for(
            NodeType::Shell,
            &serde_json::Value::Object(Default::default()),
        );
        assert_eq!(pins.len(), 3);
        assert_eq!(pins[0].id, "exec_in");
        assert_eq!(pins[1].id, "exec_out");
        assert_eq!(pins[2].id, "output");
        assert_eq!(pins[2].category, PinCategory::Data);
        assert_eq!(pins[2].value_type, Some(PinValueType::Json));
    }

    #[test]
    fn test_branch_pins_with_rules() {
        let config = serde_json::json!({
            "mode": "first_match",
            "rules": [
                {"id": "yes", "label": "Yes", "when": {}},
                {"id": "no", "label": "No", "when": {}}
            ],
            "default_rule_id": "no"
        });
        let pins = default_pins_for(NodeType::Branch, &config);
        assert_eq!(pins.len(), 3); // exec_in + 2 rules
        assert_eq!(pins[1].id, "rule:yes");
        assert_eq!(pins[1].label, "Yes");
        assert_eq!(pins[2].id, "rule:no");
        assert_eq!(pins[2].label, "No");
    }

    #[test]
    fn test_loop_pins() {
        let config = serde_json::json!({
            "max_iterations": 5,
            "body_entry": "body-node",
            "body_exit": "body-exit-node"
        });
        let pins = default_pins_for(NodeType::Loop, &config);
        assert_eq!(pins.len(), 4);
        // In pins
        assert_eq!(pins[0].id, "exec_in");
        // Out pins
        assert_eq!(pins[1].id, "body");
        assert_eq!(pins[2].id, "exit");
        assert_eq!(pins[3].id, "index");
        assert_eq!(pins[3].category, PinCategory::Data);
        assert_eq!(pins[3].value_type, Some(PinValueType::Int));
    }

    #[test]
    fn test_human_gate_pins() {
        let pins = default_pins_for(
            NodeType::HumanGate,
            &serde_json::Value::Object(Default::default()),
        );
        assert_eq!(pins.len(), 3);
        assert_eq!(pins[0].id, "exec_in");
        assert_eq!(pins[1].id, "exec_out");
        assert_eq!(pins[2].id, "action");
        assert_eq!(pins[2].category, PinCategory::Data);
        assert_eq!(pins[2].value_type, Some(PinValueType::String));
    }
}
