//! Window Registry - manages stable references to windows
//!
//! Following Playwright MCP's pattern, we assign stable references (w0, w1, ...)
//! to windows that persist across window_list calls as long as the window exists.

use std::collections::HashMap;

use crate::backend::WindowInfo;

/// Unique identifier for a window across platforms
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PlatformWindowId {
    /// X11 window ID
    X11(u32),
    /// Wayland foreign toplevel handle (opaque identifier)
    Wayland(String),
}

/// Geometry of a window
#[derive(Debug, Clone, Default)]
pub struct Geometry {
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
}

/// A window handle with stable reference
#[derive(Debug, Clone)]
pub struct WindowHandle {
    /// Stable reference ID (e.g., "w0", "w1")
    pub ref_id: String,
    /// Platform-specific window identifier
    pub platform_id: PlatformWindowId,
    /// Window title
    pub title: String,
    /// Window class/app name
    pub class: String,
    /// Window geometry
    pub geometry: Geometry,
    /// Whether the window is currently focused
    pub focused: bool,
    /// Whether the window is visible
    pub visible: bool,
}

/// Registry that maintains stable window references
pub struct WindowRegistry {
    /// Map from ref_id to window handle
    windows: HashMap<String, WindowHandle>,
    /// Map from platform ID to ref_id (for quick lookup)
    platform_to_ref: HashMap<PlatformWindowId, String>,
    /// Next reference number to assign
    next_ref: u32,
    /// Snapshot version (incremented on each update)
    version: u64,
}

impl WindowRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            windows: HashMap::new(),
            platform_to_ref: HashMap::new(),
            next_ref: 0,
            version: 0,
        }
    }

    /// Get the current snapshot version
    pub fn version(&self) -> u64 {
        self.version
    }

    /// Update the registry with a new list of windows from the backend
    ///
    /// Windows that still exist keep their references.
    /// New windows get new references.
    /// Windows that no longer exist are removed.
    pub fn update_windows(&mut self, windows: Vec<WindowInfo>) {
        self.version += 1;

        // Track which refs are still valid
        let mut seen_refs: Vec<String> = Vec::new();

        for info in windows {
            let platform_id = info.platform_id.clone();

            // Check if we already have a ref for this window
            if let Some(ref_id) = self.platform_to_ref.get(&platform_id) {
                // Update existing window
                seen_refs.push(ref_id.clone());
                if let Some(handle) = self.windows.get_mut(ref_id) {
                    handle.title = info.title;
                    handle.class = info.class;
                    handle.geometry = info.geometry;
                    handle.focused = info.focused;
                    handle.visible = info.visible;
                }
            } else {
                // New window - assign a new ref
                let ref_id = format!("w{}", self.next_ref);
                self.next_ref += 1;

                let handle = WindowHandle {
                    ref_id: ref_id.clone(),
                    platform_id: platform_id.clone(),
                    title: info.title,
                    class: info.class,
                    geometry: info.geometry,
                    focused: info.focused,
                    visible: info.visible,
                };

                seen_refs.push(ref_id.clone());
                self.platform_to_ref.insert(platform_id, ref_id.clone());
                self.windows.insert(ref_id, handle);
            }
        }

        // Remove windows that no longer exist
        let stale_refs: Vec<String> = self
            .windows
            .keys()
            .filter(|r| !seen_refs.contains(r))
            .cloned()
            .collect();

        for ref_id in stale_refs {
            if let Some(handle) = self.windows.remove(&ref_id) {
                self.platform_to_ref.remove(&handle.platform_id);
            }
        }
    }

    /// Get a window by its reference ID
    pub fn get_window(&self, ref_id: &str) -> Option<&WindowHandle> {
        self.windows.get(ref_id)
    }

    /// Get all windows
    pub fn windows(&self) -> Vec<&WindowHandle> {
        let mut windows: Vec<_> = self.windows.values().collect();
        // Sort by ref number for consistent ordering
        windows.sort_by(|a, b| {
            let a_num: u32 = a.ref_id[1..].parse().unwrap_or(0);
            let b_num: u32 = b.ref_id[1..].parse().unwrap_or(0);
            a_num.cmp(&b_num)
        });
        windows
    }
}

impl Default for WindowRegistry {
    fn default() -> Self {
        Self::new()
    }
}
