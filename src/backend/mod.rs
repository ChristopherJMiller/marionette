//! Window backend abstraction
//!
//! This module provides a platform-agnostic interface for window operations,
//! with implementations for X11 and Wayland.

mod kwin;
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

/// Detect if running on KDE Plasma
fn is_kde_plasma() -> bool {
    // Check for KDE-specific environment variables
    std::env::var("KDE_FULL_SESSION").is_ok()
        || std::env::var("KDE_SESSION_VERSION").is_ok()
        || std::env::var("XDG_CURRENT_DESKTOP")
            .map(|d| d.to_uppercase().contains("KDE"))
            .unwrap_or(false)
}

/// Detect if running on Wayland
fn is_wayland() -> bool {
    std::env::var("WAYLAND_DISPLAY").is_ok()
        || std::env::var("XDG_SESSION_TYPE")
            .map(|t| t.to_lowercase() == "wayland")
            .unwrap_or(false)
}

/// Create the appropriate backend for the current environment
pub async fn create_backend() -> anyhow::Result<Arc<dyn WindowBackend>> {
    let display_env = std::env::var("DISPLAY").ok();
    let wayland = is_wayland();
    let kde = is_kde_plasma();

    // Always need X11/XWayland for window enumeration
    if display_env.is_none() {
        anyhow::bail!("No display server detected. Set DISPLAY for X11 or XWayland.")
    }

    let x11_backend: Arc<dyn WindowBackend> = Arc::new(x11::X11Backend::new()?);

    // On KDE Wayland, use KWin backend for proper window focus
    if wayland && kde {
        tracing::info!(
            "Using KWin backend (WAYLAND_DISPLAY={}, KDE detected)",
            std::env::var("WAYLAND_DISPLAY").unwrap_or_default()
        );
        match kwin::KWinBackend::new(x11_backend.clone()).await {
            Ok(kwin_backend) => return Ok(Arc::new(kwin_backend)),
            Err(e) => {
                tracing::warn!("Failed to initialize KWin backend, falling back to X11: {}", e);
            }
        }
    }

    // Default to pure X11 backend
    tracing::info!(
        "Using X11 backend (DISPLAY={})",
        display_env.unwrap_or_default()
    );
    Ok(x11_backend)
}
