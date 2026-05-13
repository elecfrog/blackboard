//! MCP ServerHandler implementation using the official `rmcp` SDK.
//!
//! This module bridges the rmcp `ServerHandler` trait to the existing
//! `mcp_tools` module, converting between rmcp types and the internal
//! JSON-based tool dispatch.

use std::sync::{Arc, RwLock};

use bb_core::Workspace;
use rmcp::{
    model::{
        CallToolRequestParams, CallToolResult, Content, ErrorCode, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerInfo, Tool, ToolsCapability,
    },
    service::RequestContext,
    RoleServer, ServerHandler,
};
use serde_json::{json, Map, Value};

use crate::mcp_tools;

/// Blackboard MCP server handler.
///
/// Holds a shared reference to the workspace and delegates tool calls to the
/// existing `mcp_tools` module.
#[derive(Clone)]
pub struct BbMcpHandler {
    pub workspace: Arc<RwLock<Workspace>>,
}

impl BbMcpHandler {
    pub fn new(workspace: Arc<RwLock<Workspace>>) -> Self {
        Self { workspace }
    }
}

impl ServerHandler for BbMcpHandler {
    fn get_info(&self) -> ServerInfo {
        let mut caps = ServerCapabilities::default();
        caps.tools = Some(ToolsCapability::default());
        ServerInfo::new(caps).with_server_info(Implementation::new("bb", env!("CARGO_PKG_VERSION")))
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<ListToolsResult, rmcp::ErrorData>> + Send + '_
    {
        let tools = convert_tools_list();
        #[allow(clippy::field_reassign_with_default)]
        let result = {
            let mut r = ListToolsResult::default();
            r.tools = tools;
            r
        };
        std::future::ready(Ok(result))
    }

    fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl std::future::Future<Output = Result<CallToolResult, rmcp::ErrorData>> + Send + '_
    {
        let workspace = match self.workspace.read() {
            Ok(workspace) => Ok(workspace.clone()),
            Err(_) => Err(rmcp::ErrorData::internal_error(
                "workspace state lock is poisoned".to_string(),
                None,
            )),
        };
        async move {
            match workspace {
                Ok(workspace) => call_tool_bridge(&workspace, request),
                Err(err) => Err(err),
            }
        }
    }
}

/// Convert the existing `mcp_tools::tools_list()` JSON output into rmcp `Tool` types.
fn convert_tools_list() -> Vec<Tool> {
    let list_value = mcp_tools::tools_list();
    let tools_array = list_value
        .get("tools")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    tools_array
        .into_iter()
        .filter_map(|tool_value| {
            let name = tool_value.get("name")?.as_str()?.to_string();
            let description = tool_value
                .get("description")
                .and_then(Value::as_str)
                .map(|s| s.to_string());
            let input_schema = tool_value
                .get("inputSchema")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();

            let mut tool = Tool::default();
            tool.name = name.into();
            tool.description = description.map(|d| d.into());
            tool.input_schema = Arc::new(input_schema);
            Some(tool)
        })
        .collect()
}

/// Bridge an rmcp `CallToolRequestParams` to the existing `mcp_tools::handle_tool_call`.
fn call_tool_bridge(
    workspace: &Workspace,
    request: CallToolRequestParams,
) -> Result<CallToolResult, rmcp::ErrorData> {
    // Build the params object that mcp_tools::handle_tool_call expects:
    // { "name": "...", "arguments": { ... } }
    let tool_name = request.name.to_string();
    let mut params = Map::new();
    params.insert("name".to_string(), Value::String(tool_name.clone()));
    if let Some(arguments) = request.arguments {
        params.insert("arguments".to_string(), Value::Object(arguments));
    } else {
        params.insert("arguments".to_string(), json!({}));
    }

    match mcp_tools::handle_tool_call(workspace, &Value::Object(params)) {
        Ok(result) => {
            // result is { "content": [{ "type": "text", "text": "..." }] }
            // Convert to rmcp CallToolResult
            let content = extract_content_from_result(&result);
            if content.is_empty() {
                return Err(rmcp::ErrorData::internal_error(
                    format!("tool `{tool_name}` returned no MCP content"),
                    None,
                ));
            }
            Ok(CallToolResult::success(content))
        }
        Err((code, message)) => Err(map_tool_error(code, message)),
    }
}

fn map_tool_error(code: i64, message: String) -> rmcp::ErrorData {
    match code {
        -32700 => rmcp::ErrorData::parse_error(message, None),
        -32600 => rmcp::ErrorData::invalid_request(message, None),
        -32601 => rmcp::ErrorData::new(ErrorCode::METHOD_NOT_FOUND, message, None),
        -32602 => rmcp::ErrorData::invalid_params(message, None),
        -32603 => rmcp::ErrorData::internal_error(message, None),
        _ => rmcp::ErrorData::internal_error(format!("Error {code}: {message}"), None),
    }
}

/// Extract `Vec<Content>` from the mcp_tools result JSON.
///
/// The existing format is: `{ "content": [{ "type": "text", "text": "..." }] }`
fn extract_content_from_result(result: &Value) -> Vec<Content> {
    let content_array = result
        .get("content")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    content_array
        .into_iter()
        .filter_map(|item| {
            let content_type = item.get("type")?.as_str()?;
            match content_type {
                "text" => {
                    let text = item.get("text")?.as_str()?.to_string();
                    Some(Content::text(text))
                }
                _ => None,
            }
        })
        .collect()
}
