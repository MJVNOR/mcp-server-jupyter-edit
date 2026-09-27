use std::io::{stderr, IsTerminal};

use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    tool, tool_handler, tool_router,
    transport::stdio,
    ErrorData as McpError, ServerHandler, ServiceExt,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use tracing_subscriber::{self, EnvFilter};

mod notebook;
mod format;
mod io;
mod tools;

#[derive(Clone)]
struct JupyterEditService {
    tool_router: ToolRouter<Self>,
}

impl JupyterEditService {
    fn new() -> Self {
        Self {
            tool_router: Self::tool_router(),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ReadNotebookParams {
    #[schemars(description = "Absolute path to .ipynb file")]
    path: String,
    #[schemars(description = "Maximum number of lines to return (default: 100 when omitted; pass a very large number for full output)")]
    #[serde(default = "default_read_limit")]
    limit: usize,
    #[schemars(description = "Number of lines to skip from the beginning (default: 0)")]
    #[serde(default)]
    offset: usize,
}

fn default_read_limit() -> usize {
    100
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct WriteNotebookParams {
    #[schemars(description = "Absolute path to .ipynb file to create/overwrite")]
    path: String,
    #[schemars(description = "LLM-friendly markdown format string")]
    content: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct ListCellsParams {
    #[schemars(description = "Absolute path to .ipynb file")]
    path: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct GetCellParams {
    #[schemars(description = "Absolute path to .ipynb file")]
    path: String,
    #[schemars(description = "Cell ID to retrieve")]
    cell_id: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct AddCellParams {
    #[schemars(description = "Absolute path to .ipynb file")]
    path: String,
    #[schemars(description = "Cell type: code, markdown, or raw")]
    cell_type: String,
    #[schemars(description = "Cell source content")]
    content: String,
    #[schemars(description = "Insert after this cell ID; if omitted, add at end")]
    after_cell_id: Option<String>,
    #[schemars(description = "Insert after this index (0-based); if omitted, add at end")]
    after_index: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct UpdateCellParams {
    #[schemars(description = "Absolute path to .ipynb file")]
    path: String,
    #[schemars(description = "Cell ID to update")]
    cell_id: String,
    #[schemars(description = "New source content")]
    content: String,
}

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
struct DeleteCellParams {
    #[schemars(description = "Absolute path to .ipynb file")]
    path: String,
    #[schemars(description = "Cell ID to delete")]
    cell_id: String,
}

#[tool_router]
impl JupyterEditService {
    #[tool(description = "Read a Jupyter notebook and convert to LLM-friendly markdown format")]
    async fn read_notebook(&self, params: Parameters<ReadNotebookParams>) -> Result<CallToolResult, McpError> {
        tools::read_notebook(tools::ReadNotebookRequest {
            path: params.0.path,
            limit: Some(params.0.limit),
            offset: Some(params.0.offset),
        })
            .map(|s| CallToolResult::success(vec![Content::text(s)]))
            .map_err(|e| McpError::invalid_request(e.to_string(), None))
    }

    #[tool(description = "Write a complete Jupyter notebook from LLM-friendly markdown format")]
    async fn write_notebook(&self, params: Parameters<WriteNotebookParams>) -> Result<CallToolResult, McpError> {
        tools::write_notebook(tools::WriteNotebookRequest {
            path: params.0.path,
            content: params.0.content,
        })
        .map(|r| {
            CallToolResult::success(vec![Content::text(format!(
                "{} with {} warning(s)",
                r.message,
                r.warnings.len()
            ))])
        })
        .map_err(|e| McpError::invalid_request(e.to_string(), None))
    }

    #[tool(description = "List all cells in notebook with their IDs, types, and positions")]
    async fn list_cells(&self, params: Parameters<ListCellsParams>) -> Result<CallToolResult, McpError> {
        tools::list_cells(tools::ListCellsRequest { path: params.0.path })
            .map(|cells| {
                let json = serde_json::to_string_pretty(&cells).unwrap_or_default();
                CallToolResult::success(vec![Content::text(json)])
            })
            .map_err(|e| McpError::invalid_request(e.to_string(), None))
    }

    #[tool(description = "Read a specific cell's source and outputs")]
    async fn get_cell(&self, params: Parameters<GetCellParams>) -> Result<CallToolResult, McpError> {
        tools::get_cell(tools::GetCellRequest {
            path: params.0.path,
            cell_id: params.0.cell_id,
        })
        .map(|s| CallToolResult::success(vec![Content::text(s)]))
        .map_err(|e| McpError::invalid_request(e.to_string(), None))
    }

    #[tool(description = "Add a new cell to notebook")]
    async fn add_cell(&self, params: Parameters<AddCellParams>) -> Result<CallToolResult, McpError> {
        tools::add_cell(tools::AddCellRequest {
            path: params.0.path,
            cell_type: params.0.cell_type,
            content: params.0.content,
            after_cell_id: params.0.after_cell_id,
            after_index: params.0.after_index,
        })
        .map(|r| CallToolResult::success(vec![Content::text(r.message)]))
        .map_err(|e| McpError::invalid_request(e.to_string(), None))
    }

    #[tool(description = "Update source content of an existing cell")]
    async fn update_cell(&self, params: Parameters<UpdateCellParams>) -> Result<CallToolResult, McpError> {
        tools::update_cell(tools::UpdateCellRequest {
            path: params.0.path,
            cell_id: params.0.cell_id,
            content: params.0.content,
        })
        .map(|s| CallToolResult::success(vec![Content::text(s)]))
        .map_err(|e| McpError::invalid_request(e.to_string(), None))
    }

    #[tool(description = "Remove a cell from notebook by its ID")]
    async fn delete_cell(&self, params: Parameters<DeleteCellParams>) -> Result<CallToolResult, McpError> {
        tools::delete_cell(tools::DeleteCellRequest {
            path: params.0.path,
            cell_id: params.0.cell_id,
        })
        .map(|s| CallToolResult::success(vec![Content::text(s)]))
        .map_err(|e| McpError::invalid_request(e.to_string(), None))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for JupyterEditService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Provides tools for reading and editing Jupyter notebooks (.ipynb files). \
                 Use read_notebook to get the full notebook content, write_notebook to create or update \
                 entire notebooks, list_cells to see all cells, get_cell to read a specific cell, \
                 add_cell to insert new cells, update_cell to modify cell content, and delete_cell \
                 to remove cells by ID. NEVER use default Read tool of your harness as it bloats your context"
                    .to_string(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            ..Default::default()
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(tracing::Level::INFO.into()))
        .with_writer(stderr)
        .with_ansi(stderr().is_terminal())
        .init();

    let service = JupyterEditService::new().serve(stdio()).await?;
    service.waiting().await?;

    Ok(())
}
