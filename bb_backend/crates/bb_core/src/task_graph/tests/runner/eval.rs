//! Branch and loop expression tests.

use crate::task_graph::definition::types::*;
use crate::task_graph::nodes::eval::*;
use serde_json::json;

use super::fixtures::empty_context;

// ─── Branch evaluator tests ──────────────────────────────────────────────────

#[test]
fn branch_always_matches_first() {
    let config = BranchConfig {
        mode: "first_match".to_string(),
        input_ref: None,
        rules: vec![BranchRule {
            id: "always-rule".to_string(),
            label: "Always".to_string(),
            when: json!({ "op": "always" }),
        }],
        default_rule_id: "always-rule".to_string(),
    };

    let context = empty_context();
    assert_eq!(evaluate_branch(&config, &context), "always-rule");
}

#[test]
fn branch_gt_condition() {
    let config = BranchConfig {
        mode: "first_match".to_string(),
        input_ref: Some("$.nodes.smoke-task.output".to_string()),
        rules: vec![
            BranchRule {
                id: "has-failures".to_string(),
                label: "Has failures".to_string(),
                when: json!({ "path": "$.failures.length", "op": ">", "value": 0 }),
            },
            BranchRule {
                id: "clean".to_string(),
                label: "Clean".to_string(),
                when: json!({ "op": "always" }),
            },
        ],
        default_rule_id: "clean".to_string(),
    };

    // With failures
    let mut context = empty_context();
    context.node_outputs.insert(
        "smoke-task".to_string(),
        json!({ "failures": [{"page": "A", "message": "fail"}] }),
    );
    assert_eq!(evaluate_branch(&config, &context), "has-failures");

    // Without failures
    let mut context2 = empty_context();
    context2
        .node_outputs
        .insert("smoke-task".to_string(), json!({ "failures": [] }));
    assert_eq!(evaluate_branch(&config, &context2), "clean");
}

#[test]
fn branch_equals_condition() {
    let config = BranchConfig {
        mode: "first_match".to_string(),
        input_ref: Some("$.nodes.gate.output".to_string()),
        rules: vec![
            BranchRule {
                id: "approved".to_string(),
                label: "Approved".to_string(),
                when: json!({ "path": "$.result", "op": "equals", "value": "resume" }),
            },
            BranchRule {
                id: "rejected".to_string(),
                label: "Rejected".to_string(),
                when: json!({ "op": "always" }),
            },
        ],
        default_rule_id: "rejected".to_string(),
    };

    let mut context = empty_context();
    context
        .node_outputs
        .insert("gate".to_string(), json!({ "result": "resume" }));
    assert_eq!(evaluate_branch(&config, &context), "approved");
}

#[test]
fn branch_fallback_to_default() {
    let config = BranchConfig {
        mode: "first_match".to_string(),
        input_ref: Some("$.nodes.missing.output".to_string()),
        rules: vec![BranchRule {
            id: "needs-data".to_string(),
            label: "Needs data".to_string(),
            when: json!({ "path": "$.exists", "op": "exists" }),
        }],
        default_rule_id: "needs-data".to_string(),
    };

    let context = empty_context();
    // input_ref resolves to null, "exists" check fails, goes to default
    assert_eq!(evaluate_branch(&config, &context), "needs-data");
}

#[test]
fn branch_truthy_falsy() {
    let config = BranchConfig {
        mode: "first_match".to_string(),
        input_ref: Some("$.nodes.check.output".to_string()),
        rules: vec![
            BranchRule {
                id: "truthy".to_string(),
                label: "Truthy".to_string(),
                when: json!({ "path": "$.flag", "op": "truthy" }),
            },
            BranchRule {
                id: "falsy".to_string(),
                label: "Falsy".to_string(),
                when: json!({ "op": "always" }),
            },
        ],
        default_rule_id: "falsy".to_string(),
    };

    let mut ctx = empty_context();
    ctx.node_outputs
        .insert("check".to_string(), json!({ "flag": true }));
    assert_eq!(evaluate_branch(&config, &ctx), "truthy");

    let mut ctx2 = empty_context();
    ctx2.node_outputs
        .insert("check".to_string(), json!({ "flag": false }));
    assert_eq!(evaluate_branch(&config, &ctx2), "falsy");
}

#[test]
fn branch_contains_condition() {
    let config = BranchConfig {
        mode: "first_match".to_string(),
        input_ref: Some("$.nodes.data.output".to_string()),
        rules: vec![BranchRule {
            id: "has-item".to_string(),
            label: "Has item".to_string(),
            when: json!({ "path": "$.items", "op": "contains", "value": "foo" }),
        }],
        default_rule_id: "has-item".to_string(),
    };

    let mut ctx = empty_context();
    ctx.node_outputs
        .insert("data".to_string(), json!({ "items": ["foo", "bar"] }));
    assert_eq!(evaluate_branch(&config, &ctx), "has-item");
}

// ─── Loop condition tests ────────────────────────────────────────────────────

#[test]
fn loop_condition_true_when_failures_exist() {
    let config = LoopConfig {
        max_iterations: 3,
        max_iterations_ref: None,
        condition: Some(json!({
            "input_ref": "$.nodes.smoke.output",
            "path": "$.failures.length",
            "op": ">",
            "value": 0
        })),
        body_entry: "fix".to_string(),
        body_exit: "smoke".to_string(),
        on_max_iterations: Some("fail".to_string()),
    };

    let mut ctx = empty_context();
    ctx.node_outputs
        .insert("smoke".to_string(), json!({ "failures": ["x"] }));
    assert!(evaluate_loop_condition(&config, &ctx));
}

#[test]
fn loop_condition_false_when_no_failures() {
    let config = LoopConfig {
        max_iterations: 3,
        max_iterations_ref: None,
        condition: Some(json!({
            "input_ref": "$.nodes.smoke.output",
            "path": "$.failures.length",
            "op": ">",
            "value": 0
        })),
        body_entry: "fix".to_string(),
        body_exit: "smoke".to_string(),
        on_max_iterations: Some("fail".to_string()),
    };

    let mut ctx = empty_context();
    ctx.node_outputs
        .insert("smoke".to_string(), json!({ "failures": [] }));
    assert!(!evaluate_loop_condition(&config, &ctx));
}

#[test]
fn loop_no_condition_always_true() {
    let config = LoopConfig {
        max_iterations: 5,
        max_iterations_ref: None,
        condition: None,
        body_entry: "body".to_string(),
        body_exit: "body-end".to_string(),
        on_max_iterations: Some("fail".to_string()),
    };
    assert!(evaluate_loop_condition(&config, &empty_context()));
}
