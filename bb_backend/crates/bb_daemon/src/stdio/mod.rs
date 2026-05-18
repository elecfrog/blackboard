use std::sync::{Arc, RwLock};

use crate::mcp_handler::BbMcpHandler;
use bb_core::Workspace;
use rmcp::{transport::stdio, ServiceExt};

pub async fn run(workspace: Workspace) -> anyhow::Result<()> {
    let service = BbMcpHandler::new(Arc::new(RwLock::new(workspace)))
        .serve(stdio())
        .await?;
    service.waiting().await?;
    Ok(())
}

#[cfg(test)]
mod tests;
