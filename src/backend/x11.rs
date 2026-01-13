//! X11 window backend using x11rb

use async_trait::async_trait;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{self, Atom, AtomEnum, ConnectionExt, Window};
use x11rb::rust_connection::RustConnection;

use super::{WindowBackend, WindowInfo};
use crate::core::registry::{Geometry, PlatformWindowId};

/// X11 window backend
pub struct X11Backend {
    conn: RustConnection,
    root: Window,
    atoms: X11Atoms,
}

/// Cached X11 atoms for efficiency
struct X11Atoms {
    net_client_list: Atom,
    net_wm_name: Atom,
    net_active_window: Atom,
    wm_class: Atom,
    wm_name: Atom,
    utf8_string: Atom,
    net_wm_state: Atom,
    net_wm_state_hidden: Atom,
}

impl X11Backend {
    /// Create a new X11 backend
    pub fn new() -> anyhow::Result<Self> {
        let (conn, screen_num) = RustConnection::connect(None)?;
        let screen = &conn.setup().roots[screen_num];
        let root = screen.root;

        // Intern atoms we need
        let atoms = Self::intern_atoms(&conn)?;

        Ok(Self { conn, root, atoms })
    }

    fn intern_atoms(conn: &RustConnection) -> anyhow::Result<X11Atoms> {
        let net_client_list = conn.intern_atom(false, b"_NET_CLIENT_LIST")?.reply()?.atom;
        let net_wm_name = conn.intern_atom(false, b"_NET_WM_NAME")?.reply()?.atom;
        let net_active_window = conn.intern_atom(false, b"_NET_ACTIVE_WINDOW")?.reply()?.atom;
        let wm_class = conn.intern_atom(false, b"WM_CLASS")?.reply()?.atom;
        let wm_name = conn.intern_atom(false, b"WM_NAME")?.reply()?.atom;
        let utf8_string = conn.intern_atom(false, b"UTF8_STRING")?.reply()?.atom;
        let net_wm_state = conn.intern_atom(false, b"_NET_WM_STATE")?.reply()?.atom;
        let net_wm_state_hidden = conn.intern_atom(false, b"_NET_WM_STATE_HIDDEN")?.reply()?.atom;

        Ok(X11Atoms {
            net_client_list,
            net_wm_name,
            net_active_window,
            wm_class,
            wm_name,
            utf8_string,
            net_wm_state,
            net_wm_state_hidden,
        })
    }

    fn get_window_property(&self, window: Window, property: Atom, type_: Atom) -> anyhow::Result<Option<Vec<u8>>> {
        let reply = self.conn.get_property(
            false,
            window,
            property,
            type_,
            0,
            u32::MAX,
        )?.reply()?;

        if reply.value.is_empty() {
            Ok(None)
        } else {
            Ok(Some(reply.value))
        }
    }

    fn get_window_title(&self, window: Window) -> String {
        // Try _NET_WM_NAME first (UTF-8)
        if let Ok(Some(data)) = self.get_window_property(window, self.atoms.net_wm_name, self.atoms.utf8_string) {
            if let Ok(s) = String::from_utf8(data) {
                return s;
            }
        }

        // Fall back to WM_NAME
        if let Ok(Some(data)) = self.get_window_property(window, self.atoms.wm_name, AtomEnum::STRING.into()) {
            if let Ok(s) = String::from_utf8(data) {
                return s;
            }
        }

        String::new()
    }

    fn get_window_class(&self, window: Window) -> String {
        if let Ok(Some(data)) = self.get_window_property(window, self.atoms.wm_class, AtomEnum::STRING.into()) {
            // WM_CLASS is two null-separated strings: instance name and class name
            // We want the class name (second one)
            let parts: Vec<&[u8]> = data.split(|&b| b == 0).collect();
            if parts.len() >= 2 {
                if let Ok(s) = std::str::from_utf8(parts[1]) {
                    return s.to_string();
                }
            }
            if let Some(part) = parts.first() {
                if let Ok(s) = std::str::from_utf8(part) {
                    return s.to_string();
                }
            }
        }
        String::new()
    }

    fn get_window_geometry(&self, window: Window) -> anyhow::Result<Geometry> {
        let geom = self.conn.get_geometry(window)?.reply()?;

        // Translate to root window coordinates
        let translated = self.conn.translate_coordinates(window, self.root, 0, 0)?.reply()?;

        Ok(Geometry {
            x: translated.dst_x as i32,
            y: translated.dst_y as i32,
            width: geom.width as u32,
            height: geom.height as u32,
        })
    }

    fn get_active_window(&self) -> Option<Window> {
        if let Ok(Some(data)) = self.get_window_property(self.root, self.atoms.net_active_window, AtomEnum::WINDOW.into()) {
            if data.len() >= 4 {
                let window = u32::from_ne_bytes([data[0], data[1], data[2], data[3]]);
                if window != 0 {
                    return Some(window);
                }
            }
        }
        None
    }

    fn is_window_visible(&self, window: Window) -> bool {
        // Check _NET_WM_STATE for hidden state
        if let Ok(Some(data)) = self.get_window_property(window, self.atoms.net_wm_state, AtomEnum::ATOM.into()) {
            // Parse as array of atoms
            for chunk in data.chunks(4) {
                if chunk.len() == 4 {
                    let atom = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                    if atom == self.atoms.net_wm_state_hidden {
                        return false;
                    }
                }
            }
        }

        // Check if window is viewable
        if let Ok(attrs) = self.conn.get_window_attributes(window) {
            if let Ok(reply) = attrs.reply() {
                return reply.map_state == xproto::MapState::VIEWABLE;
            }
        }

        true
    }
}

#[async_trait]
impl WindowBackend for X11Backend {
    async fn list_windows(&self) -> anyhow::Result<Vec<WindowInfo>> {
        let mut windows = Vec::new();

        // Get _NET_CLIENT_LIST
        let data = match self.get_window_property(self.root, self.atoms.net_client_list, AtomEnum::WINDOW.into())? {
            Some(d) => d,
            None => return Ok(windows),
        };

        let active_window = self.get_active_window();

        // Parse window IDs (each is 4 bytes)
        for chunk in data.chunks(4) {
            if chunk.len() == 4 {
                let window_id = u32::from_ne_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);

                // Get window info
                let title = self.get_window_title(window_id);
                let class = self.get_window_class(window_id);
                let geometry = self.get_window_geometry(window_id).unwrap_or_default();
                let focused = active_window == Some(window_id);
                let visible = self.is_window_visible(window_id);

                windows.push(WindowInfo {
                    platform_id: PlatformWindowId::X11(window_id),
                    title,
                    class,
                    geometry,
                    focused,
                    visible,
                });
            }
        }

        Ok(windows)
    }

    async fn focus_window(&self, id: &PlatformWindowId) -> anyhow::Result<()> {
        let PlatformWindowId::X11(window_id) = id else {
            anyhow::bail!("X11 backend cannot handle non-X11 window IDs");
        };

        // Use _NET_ACTIVE_WINDOW client message
        let event = xproto::ClientMessageEvent::new(
            32,
            *window_id,
            self.atoms.net_active_window,
            [1, 0, 0, 0, 0], // Source indication: 1 = application
        );

        self.conn.send_event(
            false,
            self.root,
            xproto::EventMask::SUBSTRUCTURE_REDIRECT | xproto::EventMask::SUBSTRUCTURE_NOTIFY,
            event,
        )?;

        self.conn.flush()?;
        Ok(())
    }

    async fn move_window(&self, id: &PlatformWindowId, x: i32, y: i32) -> anyhow::Result<()> {
        let PlatformWindowId::X11(window_id) = id else {
            anyhow::bail!("X11 backend cannot handle non-X11 window IDs");
        };

        // Configure window position
        let values = xproto::ConfigureWindowAux::new()
            .x(x)
            .y(y);

        self.conn.configure_window(*window_id, &values)?;
        self.conn.flush()?;
        Ok(())
    }

    async fn resize_window(&self, id: &PlatformWindowId, width: u32, height: u32) -> anyhow::Result<()> {
        let PlatformWindowId::X11(window_id) = id else {
            anyhow::bail!("X11 backend cannot handle non-X11 window IDs");
        };

        // Configure window size
        let values = xproto::ConfigureWindowAux::new()
            .width(width)
            .height(height);

        self.conn.configure_window(*window_id, &values)?;
        self.conn.flush()?;
        Ok(())
    }
}

// Safety: RustConnection is Send + Sync
unsafe impl Send for X11Backend {}
unsafe impl Sync for X11Backend {}
