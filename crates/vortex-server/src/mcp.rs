//! MCP (Model Context Protocol) server.
//!
//! Exposes VORTEX capabilities as MCP tools that AI agents can call:
//!
//! | Tool                | Description                                         |
//! |---------------------|-----------------------------------------------------|
//! | `create_project`    | Create a new empty project                          |
//! | `add_clip`          | Add a clip to the timeline                          |
//! | `add_effect`        | Attach an effect to a clip                          |
//! | `analyse_video`     | Run kill/beat/scene analysis on a source file       |
//! | `render_project`    | Trigger FFmpeg render pipeline                      |
//! | `apply_style`       | Apply a named style template to the project         |
//! | `list_styles`       | Return available style templates                    |
//!
//! ## Implementation roadmap (Phase 3)
//!
//! 1. Implement JSON-RPC 2.0 transport over stdio (default MCP transport).
//! 2. Implement HTTP+SSE transport for remote agents.
//! 3. Register each tool with parameter schemas.
//! 4. Wire tool calls to `vortex-render`, `vortex-analysis`, `vortex-styles`.
//! 5. Maintain project state in a `HashMap<ProjectId, Project>` session store.

use serde::{Deserialize, Serialize};

/// MCP JSON-RPC request.
#[derive(Debug, Deserialize)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    pub method: String,
    #[serde(default)]
    pub params: serde_json::Value,
}

/// MCP JSON-RPC response.
#[derive(Debug, Serialize)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

/// MCP JSON-RPC error object.
#[derive(Debug, Serialize)]
pub struct McpError {
    pub code: i32,
    pub message: String,
}

impl McpResponse {
    pub fn ok(id: serde_json::Value, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn err(id: serde_json::Value, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(McpError { code, message: message.into() }),
        }
    }
}

/// Tool descriptor for MCP tool listing.
#[derive(Debug, Serialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}

/// The MCP server instance.
pub struct McpServer {
    host: String,
    port: u16,
}

impl McpServer {
    pub fn new(host: &str, port: u16) -> Self {
        Self {
            host: host.to_string(),
            port,
        }
    }

    /// Return all available VORTEX tools.
    pub fn tools(&self) -> Vec<ToolDescriptor> {
        vec![
            ToolDescriptor {
                name: "create_project".into(),
                description: "Create a new VORTEX project.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "name": { "type": "string", "description": "Project name" },
                        "style": { "type": "string", "description": "Style template name (optional)" }
                    },
                    "required": ["name"]
                }),
            },
            ToolDescriptor {
                name: "add_clip".into(),
                description: "Add a video clip to the project timeline.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" },
                        "source_path": { "type": "string", "description": "Absolute path to source video" },
                        "source_start": { "type": "number", "description": "Clip start time in source (seconds)" },
                        "source_end": { "type": "number", "description": "Clip end time in source (seconds)" },
                        "label": { "type": "string", "description": "Optional clip label" }
                    },
                    "required": ["project_id", "source_path", "source_start", "source_end"]
                }),
            },
            ToolDescriptor {
                name: "add_effect".into(),
                description: "Attach an effect to a clip.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" },
                        "clip_id": { "type": "string" },
                        "effect_type": {
                            "type": "string",
                            "enum": ["velocity", "zoom", "shake", "color", "flash", "chromatic", "letterbox", "vignette", "glitch"]
                        },
                        "params": { "type": "object", "description": "Effect-specific parameters" }
                    },
                    "required": ["project_id", "clip_id", "effect_type"]
                }),
            },
            ToolDescriptor {
                name: "analyse_video".into(),
                description: "Analyse a video for kill moments, beat markers, and scene cuts.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "source_path": { "type": "string" }
                    },
                    "required": ["source_path"]
                }),
            },
            ToolDescriptor {
                name: "render_project".into(),
                description: "Render a project to a video file via FFmpeg.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" },
                        "output_path": { "type": "string" },
                        "hw_accel": { "type": "string", "enum": ["none", "nvidia", "amf", "videotoolbox"] }
                    },
                    "required": ["project_id", "output_path"]
                }),
            },
            ToolDescriptor {
                name: "apply_style".into(),
                description: "Apply a named style template (aggressive, chill, cinematic) to the project.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {
                        "project_id": { "type": "string" },
                        "style_name": { "type": "string" }
                    },
                    "required": ["project_id", "style_name"]
                }),
            },
            ToolDescriptor {
                name: "list_styles".into(),
                description: "Return all available VORTEX style templates.".into(),
                input_schema: serde_json::json!({
                    "type": "object",
                    "properties": {}
                }),
            },
        ]
    }

    /// Dispatch an incoming MCP request to the appropriate handler.
    ///
    /// # TODO (Phase 3)
    /// Implement actual handlers for each tool, maintaining project state
    /// in a session map.
    pub async fn dispatch(&self, req: McpRequest) -> McpResponse {
        tracing::debug!(method = %req.method, "MCP request");

        match req.method.as_str() {
            "initialize" => McpResponse::ok(req.id, serde_json::json!({
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": "vortex", "version": env!("CARGO_PKG_VERSION") }
            })),

            "tools/list" => McpResponse::ok(req.id, serde_json::json!({
                "tools": self.tools()
            })),

            "tools/call" => {
                // TODO (Phase 3): route to individual tool handlers
                let tool_name = req.params.get("name").and_then(|v| v.as_str()).unwrap_or("unknown");
                tracing::warn!(tool = tool_name, "Tool call STUB — not yet implemented");
                McpResponse::err(req.id, -32601, format!("Tool '{tool_name}' not yet implemented"))
            }

            other => McpResponse::err(req.id, -32601, format!("Method not found: {other}")),
        }
    }

    /// Run the MCP server. Currently only stdio transport is supported.
    ///
    /// # TODO (Phase 3)
    /// - Implement HTTP+SSE transport via `axum`.
    /// - Add authentication.
    pub async fn run(&self) -> anyhow::Result<()> {
        tracing::info!(host = %self.host, port = self.port, "MCP server starting (STUB)");
        tracing::warn!("MCP server transport not yet implemented — coming in Phase 3");

        // Keep the process alive (placeholder)
        tokio::signal::ctrl_c().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mcp_initialize() {
        let server = McpServer::new("127.0.0.1", 7700);
        let req = McpRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(1),
            method: "initialize".into(),
            params: serde_json::Value::Null,
        };
        let resp = server.dispatch(req).await;
        assert!(resp.error.is_none());
        assert!(resp.result.is_some());
    }

    #[tokio::test]
    async fn mcp_tools_list() {
        let server = McpServer::new("127.0.0.1", 7700);
        let req = McpRequest {
            jsonrpc: "2.0".into(),
            id: serde_json::json!(2),
            method: "tools/list".into(),
            params: serde_json::Value::Null,
        };
        let resp = server.dispatch(req).await;
        let tools = resp.result.unwrap()["tools"].as_array().unwrap().len();
        assert!(tools >= 6);
    }
}
