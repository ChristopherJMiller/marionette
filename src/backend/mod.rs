//! Window backend abstraction
//!
//! This module provides a platform-agnostic interface for window operations,
//! with implementations for X11 and Wayland.

mod x11;

use async_trait::async_trait;
use std::sync::Arc;

use crate::core::registry::{Geometry, PlatformWindowId};

/// Information about a window from the backend
#[derive(Debug, Clone)]
pub struct WindowInfo {
    pub platform_id: PlatformWindowId,
    pub title: String,
    pub class: String,
    pub geometry: Geometry,
    pub focused: bool,
    pub visible: bool,
}

/// Trait for window backend implementations
#[async_trait]
pub trait WindowBackend: Send + Sync {
    /// List all windows
    async fn list_windows(&self) -> anyhow::Result<Vec<WindowInfo>>;

    /// Focus a window
    async fn focus_window(&self, id: &PlatformWindowId) -> anyhow::Result<()>;

    /// Move a window
    async fn move_window(&self, id: &PlatformWindowId, x: i32, y: i32) -> anyhow::Result<()>;

    /// Resize a window
    async fn resize_window(&self, id: &PlatformWindowId, width: u32, height: u32) -> anyhow::Result<()>;
}

/// Create the appropriate backend for the current environment
pub fn create_backend() -> anyhow::Result<Arc<dyn WindowBackend>> {
    // For now, we always use X11 backend
    // On Wayland with XWayland, this still works for XWayland windows (games)
    // Future: Add detection and Wayland backend for wlroots compositors

    let display_env = std::env::var("DISPLAY").ok();

    if let Some(ref disp) = display_env {
        tracing::info!("Using X11 backend (DISPLAY={})", disp);
        Ok(Arc::new(x11::X11Backend::new()?))
    } else {
        anyhow::bail!("No display server detected. Set DISPLAY for X11 or XWayland.")
    }
}
