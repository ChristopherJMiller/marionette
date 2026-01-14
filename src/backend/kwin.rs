//! KWin backend for KDE Plasma on Wayland
//!
//! This backend wraps the X11 backend for window enumeration (via XWayland)
//! but uses KWin's D-Bus scripting API for window focus, which properly
//! raises windows on Wayland instead of just requesting attention.

use async_trait::async_trait;
use std::sync::Arc;
use zbus::Connection;
use zbus::zvariant::ObjectPath;

use super::{WindowBackend, WindowInfo};
use crate::core::registry::PlatformWindowId;

/// KWin backend that uses D-Bus for focus operations
pub struct KWinBackend {
    /// Wrapped X11 backend for listing/geometry operations
    x11_backend: Arc<dyn WindowBackend>,
    /// D-Bus connection
    dbus: Connection,
}

impl KWinBackend {
    /// Create a new KWin backend
    pub async fn new(x11_backend: Arc<dyn WindowBackend>) -> anyhow::Result<Self> {
        let dbus = Connection::session().await?;
        Ok(Self { x11_backend, dbus })
    }

    /// Focus a window using KWin's scripting API
    async fn focus_via_kwin(&self, window_title: &str) -> anyhow::Result<()> {
        // KWin scripting API: load a script that finds and activates the window
        // The script uses workspace.windowList() (KDE 6) or workspace.clientList() (KDE 5)
        let script = format!(
            r#"
            (function() {{
                // Try KDE 6 API first, fall back to KDE 5
                var windows = typeof workspace.windowList === 'function'
                    ? workspace.windowList()
                    : workspace.clientList();
                for (var i = 0; i < windows.length; i++) {{
                    var w = windows[i];
                    var title = w.caption || w.title || '';
                    if (title === '{}') {{
                        workspace.activeWindow = w;  // KDE 6
                        workspace.activeClient = w;  // KDE 5 fallback
                        break;
                    }}
                }}
            }})();
            "#,
            window_title.replace('\\', "\\\\").replace('\'', "\\'").replace('"', "\\\"")
        );

        // Write script to temp file (KWin requires a file path)
        let temp_dir = std::env::temp_dir();
        let script_path = temp_dir.join(format!("marionette_focus_{}.js", std::process::id()));
        tokio::fs::write(&script_path, &script).await?;

        // Load the script via D-Bus using the Scripting interface
        let script_path_str = script_path.to_str().unwrap_or("");

        let reply = self.dbus
            .call_method(
                Some("org.kde.KWin"),
                "/Scripting",
                Some("org.kde.kwin.Scripting"),
                "loadScript",
                &(script_path_str,),
            )
            .await?;

        let script_id: i32 = reply.body().deserialize()?;

        if script_id >= 0 {
            // Run the script via its object path
            let script_obj_path = format!("/{}", script_id);
            let script_obj_path = ObjectPath::try_from(script_obj_path.as_str())?;

            self.dbus
                .call_method(
                    Some("org.kde.KWin"),
                    script_obj_path,
                    Some("org.kde.kwin.Script"),
                    "run",
                    &(),
                )
                .await?;

            // Small delay to let the script execute
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

            // Stop/unload the script (ignore errors)
            let _ = self.dbus
                .call_method(
                    Some("org.kde.KWin"),
                    ObjectPath::try_from(format!("/{}", script_id).as_str())?,
                    Some("org.kde.kwin.Script"),
                    "stop",
                    &(),
                )
                .await;
        }

        // Clean up temp file
        let _ = tokio::fs::remove_file(&script_path).await;

        Ok(())
    }
}

#[async_trait]
impl WindowBackend for KWinBackend {
    async fn list_windows(&self) -> anyhow::Result<Vec<WindowInfo>> {
        // Delegate to X11 backend - this works via XWayland
        self.x11_backend.list_windows().await
    }

    async fn focus_window(&self, id: &PlatformWindowId) -> anyhow::Result<()> {
        // First, get the window title from X11 so we can find it in KWin
        let windows = self.x11_backend.list_windows().await?;
        let window = windows
            .iter()
            .find(|w| &w.platform_id == id)
            .ok_or_else(|| anyhow::anyhow!("Window not found"))?;

        // Try KWin D-Bus focus first
        match self.focus_via_kwin(&window.title).await {
            Ok(()) => {
                tracing::debug!("Focused window via KWin D-Bus: {}", window.title);
                Ok(())
            }
            Err(e) => {
                tracing::warn!("KWin D-Bus focus failed, falling back to X11: {}", e);
                // Fall back to X11 (may only request attention, but better than nothing)
                self.x11_backend.focus_window(id).await
            }
        }
    }

    async fn move_window(&self, id: &PlatformWindowId, x: i32, y: i32) -> anyhow::Result<()> {
        // Delegate to X11 backend - this usually works for XWayland windows
        self.x11_backend.move_window(id, x, y).await
    }

    async fn resize_window(&self, id: &PlatformWindowId, width: u32, height: u32) -> anyhow::Result<()> {
        // Delegate to X11 backend
        self.x11_backend.resize_window(id, width, height).await
    }
}
