//! Built-in node taxonomy and business role registry.
//!
//! #000062 deliberately keeps executable `NodeType` small. Business roles such
//! as explorer / implementer / verifier are registry specs layered on top of
//! the existing `llm`, `shell`, and `human_gate` executors.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::task_graph::compile::channels::{ChannelKind, ChannelValueType};
use crate::task_graph::definition::pins::default_pins_for;
use crate::task_graph::definition::types::{NodePin, NodeType, PinDirection, TaskGraphNode};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeCategory {
    Terminal,
    Control,
    Runtime,
    Transform,
    Artifact,
    Integration,
    Approval,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeRole {
    ExplorerAgent,
    ImplementerAgent,
    VerifierAgent,
    ReviewerAgent,
    HandoffWriter,
    OpenCodeSession,
    CodexSession,
    ClaudeSession,
    LocalShell,
    WriteWikiDoc,
    UpdateTicket,
    FeishuNotify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeBindingKind {
    None,
    Llm,
    AgentSession,
    Shell,
    SubGraph,
    Blackboard,
    Webhook,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionResumePolicy {
    None,
    ReuseByRun,
    ReuseByNode,
    ForkFromPrevious,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeBinding {
    pub kind: RuntimeBindingKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variant: Option<String>,
    pub session_resume_policy: SessionResumePolicy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PermissionKind {
    ReadProject,
    ReadWorktree,
    WriteScoped,
    RunTests,
    Network,
    GitOperation,
    ExternalNotify,
    UpdateTicket,
    WriteWiki,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionSpec {
    pub kind: PermissionKind,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactOutputSpec {
    pub name: String,
    pub value_type: ChannelValueType,
    pub channel_kind: ChannelKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSpec {
    pub id: String,
    pub display_name: String,
    pub category: NodeCategory,
    pub node_type: NodeType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub role: Option<NodeRole>,
    pub input_pins: Vec<NodePin>,
    pub output_pins: Vec<NodePin>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime: Option<RuntimeBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub permissions: Vec<PermissionSpec>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub artifact_outputs: Vec<ArtifactOutputSpec>,
}

pub fn builtin_node_specs() -> Vec<NodeSpec> {
    let empty = Value::Object(Default::default());
    vec![
        base_spec(
            "start",
            "Start",
            NodeCategory::Terminal,
            NodeType::Start,
            &empty,
        ),
        base_spec("end", "End", NodeCategory::Terminal, NodeType::End, &empty),
        base_spec(
            "branch",
            "Branch",
            NodeCategory::Control,
            NodeType::Branch,
            &empty,
        ),
        base_spec(
            "loop",
            "Loop",
            NodeCategory::Control,
            NodeType::Loop,
            &empty,
        ),
        base_spec(
            "human_gate",
            "Human Gate",
            NodeCategory::Approval,
            NodeType::HumanGate,
            &empty,
        ),
        base_spec(
            "input_var",
            "Input Variable",
            NodeCategory::Transform,
            NodeType::InputVar,
            &empty,
        ),
        base_spec("llm", "LLM", NodeCategory::Runtime, NodeType::Llm, &empty),
        base_spec(
            "plan",
            "Plan",
            NodeCategory::Transform,
            NodeType::Plan,
            &empty,
        ),
        base_spec(
            "llm_mutation",
            "LLM Mutation",
            NodeCategory::Transform,
            NodeType::LlmMutation,
            &empty,
        ),
        base_spec(
            "intent_extract",
            "Intent Extract",
            NodeCategory::Transform,
            NodeType::IntentExtract,
            &empty,
        ),
        base_spec(
            "kb_plan",
            "KB Plan",
            NodeCategory::Transform,
            NodeType::KbPlan,
            &empty,
        ),
        base_spec(
            "manifest_merge",
            "Manifest Merge",
            NodeCategory::Transform,
            NodeType::ManifestMerge,
            &empty,
        ),
        base_spec(
            "schema_validate",
            "Schema Validate",
            NodeCategory::Transform,
            NodeType::SchemaValidate,
            &empty,
        ),
        base_spec(
            "shell",
            "Shell",
            NodeCategory::Runtime,
            NodeType::Shell,
            &empty,
        ),
        runtime_llm_role(
            NodeRole::ExplorerAgent,
            "explorer_agent",
            "Explorer Agent",
            &[PermissionKind::ReadProject, PermissionKind::ReadWorktree],
            &[artifact(
                "findings",
                ChannelValueType::Json,
                ChannelKind::Topic,
            )],
        ),
        runtime_llm_role(
            NodeRole::ImplementerAgent,
            "implementer_agent",
            "Implementer Agent",
            &[
                PermissionKind::ReadProject,
                PermissionKind::ReadWorktree,
                PermissionKind::WriteScoped,
            ],
            &[
                artifact("diff", ChannelValueType::Diff, ChannelKind::LastValue),
                artifact(
                    "implementation_log",
                    ChannelValueType::RuntimeLog,
                    ChannelKind::Topic,
                ),
            ],
        ),
        runtime_llm_role(
            NodeRole::VerifierAgent,
            "verifier_agent",
            "Verifier Agent",
            &[
                PermissionKind::ReadProject,
                PermissionKind::ReadWorktree,
                PermissionKind::RunTests,
            ],
            &[artifact(
                "test_result",
                ChannelValueType::TestResult,
                ChannelKind::LastValue,
            )],
        ),
        runtime_llm_role(
            NodeRole::ReviewerAgent,
            "reviewer_agent",
            "Reviewer Agent",
            &[PermissionKind::ReadProject, PermissionKind::ReadWorktree],
            &[artifact(
                "review_comments",
                ChannelValueType::ReviewComment,
                ChannelKind::Topic,
            )],
        ),
        runtime_llm_role(
            NodeRole::HandoffWriter,
            "handoff_writer",
            "Handoff Writer",
            &[PermissionKind::ReadProject, PermissionKind::UpdateTicket],
            &[artifact(
                "handoff_summary",
                ChannelValueType::HandoffSummary,
                ChannelKind::LastValue,
            )],
        ),
        runtime_llm_role(
            NodeRole::OpenCodeSession,
            "opencode_session",
            "OpenCode Session",
            &[PermissionKind::ReadProject, PermissionKind::ReadWorktree],
            &[artifact(
                "runtime_log",
                ChannelValueType::RuntimeLog,
                ChannelKind::Topic,
            )],
        ),
        runtime_llm_role(
            NodeRole::CodexSession,
            "codex_session",
            "Codex Session",
            &[PermissionKind::ReadProject, PermissionKind::ReadWorktree],
            &[artifact(
                "runtime_log",
                ChannelValueType::RuntimeLog,
                ChannelKind::Topic,
            )],
        ),
        shell_role(),
        blackboard_role(
            NodeRole::WriteWikiDoc,
            "write_wiki_doc",
            "Write Wiki Doc",
            PermissionKind::WriteWiki,
            artifact(
                "wiki_ref",
                ChannelValueType::WikiRef,
                ChannelKind::ArtifactRef,
            ),
        ),
        blackboard_role(
            NodeRole::UpdateTicket,
            "update_ticket",
            "Update Ticket",
            PermissionKind::UpdateTicket,
            artifact(
                "ticket_ref",
                ChannelValueType::TicketRef,
                ChannelKind::ArtifactRef,
            ),
        ),
    ]
}

pub fn node_spec_for(node: &TaskGraphNode) -> Option<NodeSpec> {
    let role = node_role_from_config(&node.config);
    builtin_node_specs().into_iter().find(|spec| {
        if let Some(role) = role {
            spec.role == Some(role)
        } else {
            spec.role.is_none() && spec.node_type == node.node_type
        }
    })
}

pub fn node_role_from_config(config: &Value) -> Option<NodeRole> {
    config
        .get("role")
        .and_then(Value::as_str)
        .and_then(NodeRole::from_config_value)
}

impl NodeRole {
    pub fn from_config_value(value: &str) -> Option<Self> {
        match value {
            "explorer_agent" => Some(Self::ExplorerAgent),
            "implementer_agent" => Some(Self::ImplementerAgent),
            "verifier_agent" => Some(Self::VerifierAgent),
            "reviewer_agent" => Some(Self::ReviewerAgent),
            "handoff_writer" => Some(Self::HandoffWriter),
            "opencode_session" => Some(Self::OpenCodeSession),
            "codex_session" => Some(Self::CodexSession),
            "claude_session" => Some(Self::ClaudeSession),
            "local_shell" => Some(Self::LocalShell),
            "write_wiki_doc" => Some(Self::WriteWikiDoc),
            "update_ticket" => Some(Self::UpdateTicket),
            "feishu_notify" => Some(Self::FeishuNotify),
            _ => None,
        }
    }
}

pub fn node_category_for(node_type: NodeType) -> NodeCategory {
    match node_type {
        NodeType::Start | NodeType::End => NodeCategory::Terminal,
        NodeType::Branch | NodeType::Loop | NodeType::InputVar => NodeCategory::Control,
        NodeType::HumanGate => NodeCategory::Approval,
        NodeType::Llm | NodeType::Shell => NodeCategory::Runtime,
        NodeType::LlmMutation
        | NodeType::Plan
        | NodeType::IntentExtract
        | NodeType::KbPlan
        | NodeType::ManifestMerge
        | NodeType::SchemaValidate => NodeCategory::Transform,
        NodeType::SubGraph => NodeCategory::Artifact,
    }
}

fn base_spec(
    id: &str,
    display_name: &str,
    category: NodeCategory,
    node_type: NodeType,
    config: &Value,
) -> NodeSpec {
    let pins = default_pins_for(node_type, config);
    let (input_pins, output_pins): (Vec<_>, Vec<_>) = pins
        .into_iter()
        .partition(|pin| matches!(pin.direction, PinDirection::In));
    NodeSpec {
        id: id.to_string(),
        display_name: display_name.to_string(),
        category,
        node_type,
        role: None,
        input_pins,
        output_pins,
        runtime: runtime_for_node_type(node_type),
        permissions: permissions_for_node_type(node_type),
        artifact_outputs: Vec::new(),
    }
}

fn runtime_llm_role(
    role: NodeRole,
    id: &str,
    display_name: &str,
    permissions: &[PermissionKind],
    artifacts: &[ArtifactOutputSpec],
) -> NodeSpec {
    let mut spec = base_spec(
        id,
        display_name,
        NodeCategory::Runtime,
        NodeType::Llm,
        &Value::Object(Default::default()),
    );
    spec.role = Some(role);
    spec.runtime = Some(RuntimeBinding {
        kind: RuntimeBindingKind::AgentSession,
        provider: Some(default_provider_for_role(role).to_string()),
        profile: Some(id.to_string()),
        model: None,
        variant: None,
        session_resume_policy: SessionResumePolicy::ReuseByNode,
    });
    spec.permissions = permissions
        .iter()
        .copied()
        .map(required_permission)
        .collect();
    spec.artifact_outputs = artifacts.to_vec();
    spec
}

fn shell_role() -> NodeSpec {
    let mut spec = base_spec(
        "local_shell",
        "Local Shell",
        NodeCategory::Runtime,
        NodeType::Shell,
        &Value::Object(Default::default()),
    );
    spec.role = Some(NodeRole::LocalShell);
    spec.runtime = Some(RuntimeBinding {
        kind: RuntimeBindingKind::Shell,
        provider: Some("local".to_string()),
        profile: None,
        model: None,
        variant: None,
        session_resume_policy: SessionResumePolicy::None,
    });
    spec.permissions = vec![
        required_permission(PermissionKind::ReadWorktree),
        required_permission(PermissionKind::RunTests),
    ];
    spec.artifact_outputs = vec![artifact(
        "runtime_log",
        ChannelValueType::RuntimeLog,
        ChannelKind::Topic,
    )];
    spec
}

fn blackboard_role(
    role: NodeRole,
    id: &str,
    display_name: &str,
    permission: PermissionKind,
    artifact: ArtifactOutputSpec,
) -> NodeSpec {
    NodeSpec {
        id: id.to_string(),
        display_name: display_name.to_string(),
        category: NodeCategory::Artifact,
        node_type: NodeType::SubGraph,
        role: Some(role),
        input_pins: Vec::new(),
        output_pins: Vec::new(),
        runtime: Some(RuntimeBinding {
            kind: RuntimeBindingKind::Blackboard,
            provider: Some("blackboard".to_string()),
            profile: Some(id.to_string()),
            model: None,
            variant: None,
            session_resume_policy: SessionResumePolicy::None,
        }),
        permissions: vec![required_permission(permission)],
        artifact_outputs: vec![artifact],
    }
}

fn runtime_for_node_type(node_type: NodeType) -> Option<RuntimeBinding> {
    match node_type {
        NodeType::Llm => Some(RuntimeBinding {
            kind: RuntimeBindingKind::Llm,
            provider: Some("codex".to_string()),
            profile: Some("native".to_string()),
            model: None,
            variant: None,
            session_resume_policy: SessionResumePolicy::ReuseByNode,
        }),
        NodeType::Plan => Some(RuntimeBinding {
            kind: RuntimeBindingKind::Llm,
            provider: Some("codex".to_string()),
            profile: Some("native".to_string()),
            model: None,
            variant: None,
            session_resume_policy: SessionResumePolicy::ReuseByNode,
        }),
        NodeType::LlmMutation => Some(RuntimeBinding {
            kind: RuntimeBindingKind::Blackboard,
            provider: Some("task_graph".to_string()),
            profile: None,
            model: None,
            variant: None,
            session_resume_policy: SessionResumePolicy::None,
        }),
        NodeType::IntentExtract => Some(RuntimeBinding {
            kind: RuntimeBindingKind::Blackboard,
            provider: Some("task_graph".to_string()),
            profile: None,
            model: None,
            variant: None,
            session_resume_policy: SessionResumePolicy::None,
        }),
        NodeType::KbPlan => Some(RuntimeBinding {
            kind: RuntimeBindingKind::Blackboard,
            provider: Some("task_graph".to_string()),
            profile: None,
            model: None,
            variant: None,
            session_resume_policy: SessionResumePolicy::None,
        }),
        NodeType::ManifestMerge => Some(RuntimeBinding {
            kind: RuntimeBindingKind::Blackboard,
            provider: Some("task_graph".to_string()),
            profile: None,
            model: None,
            variant: None,
            session_resume_policy: SessionResumePolicy::None,
        }),
        NodeType::SchemaValidate => Some(RuntimeBinding {
            kind: RuntimeBindingKind::Blackboard,
            provider: Some("task_graph".to_string()),
            profile: None,
            model: None,
            variant: None,
            session_resume_policy: SessionResumePolicy::None,
        }),
        NodeType::Shell => Some(RuntimeBinding {
            kind: RuntimeBindingKind::Shell,
            provider: Some("local".to_string()),
            profile: None,
            model: None,
            variant: None,
            session_resume_policy: SessionResumePolicy::None,
        }),
        NodeType::SubGraph => Some(RuntimeBinding {
            kind: RuntimeBindingKind::SubGraph,
            provider: Some("task_graph".to_string()),
            profile: None,
            model: None,
            variant: None,
            session_resume_policy: SessionResumePolicy::None,
        }),
        _ => None,
    }
}

fn permissions_for_node_type(node_type: NodeType) -> Vec<PermissionSpec> {
    match node_type {
        NodeType::Llm | NodeType::Plan => vec![required_permission(PermissionKind::ReadProject)],
        NodeType::LlmMutation
        | NodeType::IntentExtract
        | NodeType::KbPlan
        | NodeType::ManifestMerge
        | NodeType::SchemaValidate => {
            vec![required_permission(PermissionKind::ReadProject)]
        }
        NodeType::Shell => vec![required_permission(PermissionKind::ReadWorktree)],
        NodeType::SubGraph => vec![required_permission(PermissionKind::ReadProject)],
        _ => Vec::new(),
    }
}

fn required_permission(kind: PermissionKind) -> PermissionSpec {
    PermissionSpec {
        kind,
        required: true,
        scope: None,
    }
}

fn artifact(
    name: &str,
    value_type: ChannelValueType,
    channel_kind: ChannelKind,
) -> ArtifactOutputSpec {
    ArtifactOutputSpec {
        name: name.to_string(),
        value_type,
        channel_kind,
    }
}

fn default_provider_for_role(role: NodeRole) -> &'static str {
    match role {
        NodeRole::OpenCodeSession => "opencode",
        NodeRole::CodexSession => "codex",
        NodeRole::ClaudeSession => "claude",
        _ => "codex",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn registry_contains_swe_business_roles() {
        let specs = builtin_node_specs();
        let explorer = specs
            .iter()
            .find(|spec| spec.role == Some(NodeRole::ExplorerAgent))
            .unwrap();

        assert_eq!(explorer.node_type, NodeType::Llm);
        assert_eq!(explorer.category, NodeCategory::Runtime);
        assert!(explorer
            .permissions
            .iter()
            .any(|permission| permission.kind == PermissionKind::ReadWorktree));
        assert!(explorer
            .artifact_outputs
            .iter()
            .any(|artifact| artifact.name == "findings"));
    }

    #[test]
    fn node_spec_for_prefers_role_over_raw_type() {
        let node = TaskGraphNode {
            id: "review".to_string(),
            node_type: NodeType::Llm,
            label: "Review".to_string(),
            description: None,
            position: None,
            config: json!({ "role": "reviewer_agent" }),
            pins: Vec::new(),
        };

        let spec = node_spec_for(&node).unwrap();
        assert_eq!(spec.role, Some(NodeRole::ReviewerAgent));
        assert!(spec
            .artifact_outputs
            .iter()
            .any(|artifact| artifact.value_type == ChannelValueType::ReviewComment));
    }

    #[test]
    fn raw_shell_has_runtime_binding_and_permissions() {
        let node = TaskGraphNode {
            id: "status".to_string(),
            node_type: NodeType::Shell,
            label: "Status".to_string(),
            description: None,
            position: None,
            config: json!({}),
            pins: Vec::new(),
        };

        let spec = node_spec_for(&node).unwrap();
        assert_eq!(
            spec.runtime.as_ref().map(|runtime| runtime.kind),
            Some(RuntimeBindingKind::Shell)
        );
        assert!(spec
            .permissions
            .iter()
            .any(|permission| permission.kind == PermissionKind::ReadWorktree));
    }
}
