//! MCP Server implementation for Marionette
//!
//! This module implements the Model Context Protocol server that exposes
//! window manipulation tools to AI assistants.

use rmcp::{
    handler::server::{tool::ToolRouter, wrapper::Parameters},
    model::*,
    schemars, serde,
    service::RequestContext,
    tool, tool_handler, tool_router, ErrorData as McpError, RoleServer, ServerHandler,
};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;

use crate::backend::WindowBackend;
use crate::core::registry::WindowRegistry;

/// Parameters for window_list tool
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WindowListParams {
    /// Filter windows by title (case-insensitive substring match)
    #[serde(default)]
    pub title_filter: Option<String>,
    /// Filter windows by class/app name
    #[serde(default)]
    pub class_filter: Option<String>,
}

/// Parameters for window_snapshot tool
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WindowSnapshotParams {
    /// Window reference (e.g., "w0") from window_list
    pub r#ref: String,
}

/// Parameters for window_focus tool
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WindowFocusParams {
    /// Window reference (e.g., "w0") from window_list
    pub r#ref: String,
    /// Human-readable description for verification
    #[serde(default)]
    pub description: Option<String>,
}

/// Parameters for window_screenshot tool
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WindowScreenshotParams {
    /// Window reference (e.g., "w0") from window_list
    pub r#ref: String,
    /// Output format: "base64" (default) or "file"
    #[serde(default = "default_format")]
    pub format: String,
}

fn default_format() -> String {
    "base64".to_string()
}

/// Parameters for window_click tool
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WindowClickParams {
    /// Window reference (e.g., "w0") from window_list
    pub r#ref: String,
    /// X coordinate within the window
    pub x: i32,
    /// Y coordinate within the window
    pub y: i32,
    /// Mouse button: "left" (default), "right", "middle"
    #[serde(default = "default_button")]
    pub button: String,
    /// Human-readable description of what's being clicked
    #[serde(default)]
    pub description: Option<String>,
}

fn default_button() -> String {
    "left".to_string()
}

/// Parameters for window_type tool
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WindowTypeParams {
    /// Text to type
    pub text: String,
    /// Delay between keystrokes in milliseconds
    #[serde(default = "default_delay")]
    pub delay_ms: u32,
}

fn default_delay() -> u32 {
    12
}

/// Parameters for window_key tool
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WindowKeyParams {
    /// Key to press (e.g., "Return", "Escape", "Tab", "a", "F1")
    pub key: String,
    /// Modifier keys to hold: "ctrl", "alt", "shift", "super"
    #[serde(default)]
    pub modifiers: Vec<String>,
}

/// Parameters for window_move tool
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WindowMoveParams {
    /// Window reference (e.g., "w0") from window_list
    pub r#ref: String,
    /// New X position
    pub x: i32,
    /// New Y position
    pub y: i32,
}

/// Parameters for window_resize tool
#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct WindowResizeParams {
    /// Window reference (e.g., "w0") from window_list
    pub r#ref: String,
    /// New width
    pub width: u32,
    /// New height
    pub height: u32,
}

/// Marionette MCP Server
///
/// Provides window manipulation tools for AI assistants on Linux.
#[derive(Clone)]
pub struct MarionetteServer {
    /// Window registry for tracking windows and their references
    registry: Arc<RwLock<WindowRegistry>>,
    /// Platform-specific window backend
    backend: Arc<dyn WindowBackend>,
    /// MCP tool router
    tool_router: ToolRouter<MarionetteServer>,
}

#[tool_router]
impl MarionetteServer {
    /// Create a new Marionette server
    pub fn new() -> anyhow::Result<Self> {
        let backend = crate::backend::create_backend()?;

        Ok(Self {
            registry: Arc::new(RwLock::new(WindowRegistry::new())),
            backend,
            tool_router: Self::tool_router(),
        })
    }

    #[tool(description = "List all windows with their references and metadata. Returns window refs (w0, w1, ...) that can be used with other tools.")]
    async fn window_list(
        &self,
        params: Parameters<WindowListParams>,
    ) -> Result<CallToolResult, McpError> {
        // Refresh window list from backend
        let windows = match self.backend.list_windows().await {
            Ok(windows) => windows,
            Err(e) => {
                return Ok(CallToolResult::error(vec![Content::text(json!({
                    "error": "Failed to list windows",
                    "details": e.to_string()
                }).to_string())]));
            }
        };

        // Update registry with new windows
        let mut registry = self.registry.write().await;
        registry.update_windows(windows);

        // Get filtered window list
        let window_list: Vec<serde_json::Value> = registry
            .windows()
            .iter()
            .filter(|w| {
                let title_match = params.0.title_filter.as_ref().is_none_or(|f| {
                    w.title.to_lowercase().contains(&f.to_lowercase())
                });
                let class_match = params.0.class_filter.as_ref().is_none_or(|f| {
                    w.class.to_lowercase().contains(&f.to_lowercase())
                });
                title_match && class_match
            })
            .map(|w| {
                json!({
                    "ref": w.ref_id,
                    "title": w.title,
                    "class": w.class,
                    "geometry": {
                        "x": w.geometry.x,
                        "y": w.geometry.y,
                        "width": w.geometry.width,
                        "height": w.geometry.height
                    },
                    "focused": w.focused,
                    "visible": w.visible
                })
            })
            .collect();

        let result = json!({
            "windows": window_list,
            "count": window_list.len(),
            "snapshot_version": registry.version()
        });

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap(),
        )]))
    }

    #[tool(description = "Get detailed snapshot of a specific window's current state")]
    async fn window_snapshot(
        &self,
        params: Parameters<WindowSnapshotParams>,
    ) -> Result<CallToolResult, McpError> {
        let registry = self.registry.read().await;

        match registry.get_window(&params.0.r#ref) {
            Some(window) => {
                let result = json!({
                    "ref": window.ref_id,
                    "title": window.title,
                    "class": window.class,
                    "geometry": {
                        "x": window.geometry.x,
                        "y": window.geometry.y,
                        "width": window.geometry.width,
                        "height": window.geometry.height
                    },
                    "focused": window.focused,
                    "visible": window.visible,
                    "platform_id": format!("{:?}", window.platform_id)
                });

                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            None => {
                Ok(CallToolResult::error(vec![Content::text(json!({
                    "error": "Window not found",
                    "ref": params.0.r#ref,
                    "suggestion": "Run window_list to get current window references"
                }).to_string())]))
            }
        }
    }

    #[tool(description = "Focus/activate a specific window, bringing it to the foreground")]
    async fn window_focus(
        &self,
        params: Parameters<WindowFocusParams>,
    ) -> Result<CallToolResult, McpError> {
        let registry = self.registry.read().await;

        let window = match registry.get_window(&params.0.r#ref) {
            Some(w) => w.clone(),
            None => {
                return Ok(CallToolResult::error(vec![Content::text(json!({
                    "error": "Window not found",
                    "ref": params.0.r#ref,
                    "suggestion": "Run window_list to get current window references"
                }).to_string())]));
            }
        };
        drop(registry);

        match self.backend.focus_window(&window.platform_id).await {
            Ok(()) => {
                let result = json!({
                    "success": true,
                    "ref": params.0.r#ref,
                    "title": window.title,
                    "message": format!("Focused window: {}", window.title)
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                Ok(CallToolResult::error(vec![Content::text(json!({
                    "error": "Failed to focus window",
                    "ref": params.0.r#ref,
                    "details": e.to_string()
                }).to_string())]))
            }
        }
    }

    #[tool(description = "Capture a screenshot of a specific window")]
    async fn window_screenshot(
        &self,
        params: Parameters<WindowScreenshotParams>,
    ) -> Result<CallToolResult, McpError> {
        let registry = self.registry.read().await;

        let window = match registry.get_window(&params.0.r#ref) {
            Some(w) => w.clone(),
            None => {
                return Ok(CallToolResult::error(vec![Content::text(json!({
                    "error": "Window not found",
                    "ref": params.0.r#ref,
                    "suggestion": "Run window_list to get current window references"
                }).to_string())]));
            }
        };
        drop(registry);

        match crate::screenshot::capture_window(&window.platform_id).await {
            Ok(image_data) => {
                if params.0.format == "file" {
                    // Save to temp file
                    let path = std::env::temp_dir().join(format!("marionette_{}_{}.png", params.0.r#ref, std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs()));
                    if let Err(e) = std::fs::write(&path, &image_data) {
                        return Ok(CallToolResult::error(vec![Content::text(json!({
                            "error": "Failed to save screenshot",
                            "details": e.to_string()
                        }).to_string())]));
                    }
                    let result = json!({
                        "success": true,
                        "ref": params.0.r#ref,
                        "path": path.to_string_lossy(),
                        "size_bytes": image_data.len()
                    });
                    Ok(CallToolResult::success(vec![Content::text(
                        serde_json::to_string_pretty(&result).unwrap(),
                    )]))
                } else {
                    // Return base64
                    let base64_data = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &image_data);
                    Ok(CallToolResult::success(vec![
                        Content::image(base64_data, "image/png")
                    ]))
                }
            }
            Err(e) => {
                Ok(CallToolResult::error(vec![Content::text(json!({
                    "error": "Failed to capture screenshot",
                    "ref": params.0.r#ref,
                    "details": e.to_string()
                }).to_string())]))
            }
        }
    }

    #[tool(description = "Click at coordinates within a window")]
    async fn window_click(
        &self,
        params: Parameters<WindowClickParams>,
    ) -> Result<CallToolResult, McpError> {
        let registry = self.registry.read().await;

        let window = match registry.get_window(&params.0.r#ref) {
            Some(w) => w.clone(),
            None => {
                return Ok(CallToolResult::error(vec![Content::text(json!({
                    "error": "Window not found",
                    "ref": params.0.r#ref,
                    "suggestion": "Run window_list to get current window references"
                }).to_string())]));
            }
        };
        drop(registry);

        // Convert window-relative to screen-absolute coordinates
        let screen_x = window.geometry.x + params.0.x;
        let screen_y = window.geometry.y + params.0.y;

        match crate::input::click(screen_x, screen_y, &params.0.button).await {
            Ok(()) => {
                let result = json!({
                    "success": true,
                    "ref": params.0.r#ref,
                    "window_coords": { "x": params.0.x, "y": params.0.y },
                    "screen_coords": { "x": screen_x, "y": screen_y },
                    "button": params.0.button,
                    "description": params.0.description
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                Ok(CallToolResult::error(vec![Content::text(json!({
                    "error": "Failed to click",
                    "details": e.to_string()
                }).to_string())]))
            }
        }
    }

    #[tool(description = "Type text into the focused window")]
    async fn window_type(
        &self,
        params: Parameters<WindowTypeParams>,
    ) -> Result<CallToolResult, McpError> {
        match crate::input::type_text(&params.0.text, params.0.delay_ms).await {
            Ok(()) => {
                let result = json!({
                    "success": true,
                    "text_length": params.0.text.len(),
                    "delay_ms": params.0.delay_ms
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                Ok(CallToolResult::error(vec![Content::text(json!({
                    "error": "Failed to type text",
                    "details": e.to_string()
                }).to_string())]))
            }
        }
    }

    #[tool(description = "Press a key or key combination")]
    async fn window_key(
        &self,
        params: Parameters<WindowKeyParams>,
    ) -> Result<CallToolResult, McpError> {
        match crate::input::key_press(&params.0.key, &params.0.modifiers).await {
            Ok(()) => {
                let result = json!({
                    "success": true,
                    "key": params.0.key,
                    "modifiers": params.0.modifiers
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                Ok(CallToolResult::error(vec![Content::text(json!({
                    "error": "Failed to press key",
                    "details": e.to_string()
                }).to_string())]))
            }
        }
    }

    #[tool(description = "Move window to a new position")]
    async fn window_move(
        &self,
        params: Parameters<WindowMoveParams>,
    ) -> Result<CallToolResult, McpError> {
        let registry = self.registry.read().await;

        let window = match registry.get_window(&params.0.r#ref) {
            Some(w) => w.clone(),
            None => {
                return Ok(CallToolResult::error(vec![Content::text(json!({
                    "error": "Window not found",
                    "ref": params.0.r#ref,
                    "suggestion": "Run window_list to get current window references"
                }).to_string())]));
            }
        };
        drop(registry);

        match self.backend.move_window(&window.platform_id, params.0.x, params.0.y).await {
            Ok(()) => {
                let result = json!({
                    "success": true,
                    "ref": params.0.r#ref,
                    "new_position": { "x": params.0.x, "y": params.0.y }
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                Ok(CallToolResult::error(vec![Content::text(json!({
                    "error": "Failed to move window",
                    "details": e.to_string()
                }).to_string())]))
            }
        }
    }

    #[tool(description = "Resize window")]
    async fn window_resize(
        &self,
        params: Parameters<WindowResizeParams>,
    ) -> Result<CallToolResult, McpError> {
        let registry = self.registry.read().await;

        let window = match registry.get_window(&params.0.r#ref) {
            Some(w) => w.clone(),
            None => {
                return Ok(CallToolResult::error(vec![Content::text(json!({
                    "error": "Window not found",
                    "ref": params.0.r#ref,
                    "suggestion": "Run window_list to get current window references"
                }).to_string())]));
            }
        };
        drop(registry);

        match self.backend.resize_window(&window.platform_id, params.0.width, params.0.height).await {
            Ok(()) => {
                let result = json!({
                    "success": true,
                    "ref": params.0.r#ref,
                    "new_size": { "width": params.0.width, "height": params.0.height }
                });
                Ok(CallToolResult::success(vec![Content::text(
                    serde_json::to_string_pretty(&result).unwrap(),
                )]))
            }
            Err(e) => {
                Ok(CallToolResult::error(vec![Content::text(json!({
                    "error": "Failed to resize window",
                    "details": e.to_string()
                }).to_string())]))
            }
        }
    }
}

#[tool_handler]
impl ServerHandler for MarionetteServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::V_2024_11_05,
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .build(),
            server_info: Implementation::from_build_env(),
            instructions: Some(
                "Marionette provides window manipulation tools for Linux desktops. \
                 Use window_list to discover windows, then use the returned refs (w0, w1, ...) \
                 with other tools for screenshots, input, and window management.".to_string()
            ),
        }
    }

    async fn initialize(
        &self,
        _request: InitializeRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        Ok(self.get_info())
    }
}
