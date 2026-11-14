//! A bare X11 top-level window to host a plugin GUI, through Xwayland on a Wayland desktop.
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{self, ConnectionExt as _, EventMask, ImageFormat, WindowClass};
use x11rb::xcb_ffi::XCBConnection;
use x11rb::wrapper::ConnectionExt as _;

pub struct HostWindow {
    conn: XCBConnection,
    pub id: u32,
    wm_delete: u32,
    pub closed_by_user: bool,
}

impl HostWindow {
    pub fn open(title: &str, w: u32, h: u32) -> Result<HostWindow, String> {
        let (conn, screen_num) = XCBConnection::connect(None).map_err(|e| format!("x11 connect: {e}"))?;
        let screen = &conn.setup().roots[screen_num];
        let id = conn.generate_id().map_err(|e| e.to_string())?;
        let aux = xproto::CreateWindowAux::new()
            .background_pixel(screen.black_pixel)
            .event_mask(EventMask::STRUCTURE_NOTIFY);
        conn.create_window(x11rb::COPY_DEPTH_FROM_PARENT, id, screen.root, 0, 0, w.max(1) as u16, h.max(1) as u16, 0,
            WindowClass::INPUT_OUTPUT, 0, &aux).map_err(|e| e.to_string())?;
        conn.change_property8(xproto::PropMode::REPLACE, id, xproto::AtomEnum::WM_NAME, xproto::AtomEnum::STRING, title.as_bytes())
            .map_err(|e| e.to_string())?;
        let protocols = conn.intern_atom(false, b"WM_PROTOCOLS").map_err(|e| e.to_string())?.reply().map_err(|e| e.to_string())?.atom;
        let wm_delete = conn.intern_atom(false, b"WM_DELETE_WINDOW").map_err(|e| e.to_string())?.reply().map_err(|e| e.to_string())?.atom;
        conn.change_property32(xproto::PropMode::REPLACE, id, protocols, xproto::AtomEnum::ATOM, &[wm_delete]).map_err(|e| e.to_string())?;
        conn.map_window(id).map_err(|e| e.to_string())?;
        conn.flush().map_err(|e| e.to_string())?;
        Ok(HostWindow { conn, id, wm_delete, closed_by_user: false })
    }

    pub fn resize(&self, w: u32, h: u32) {
        let _ = self.conn.configure_window(self.id, &xproto::ConfigureWindowAux::new().width(w).height(h));
        let _ = self.conn.flush();
    }

    /// Grows or shrinks this window to the plugin's own child window. Some GUIs never ask for a size
    /// (LV2 ui:noUserResize, e.g. Dragonfly), so their child's geometry is the only truth.
    pub fn fit_to_child(&self) -> Option<(u32, u32)> {
        let tree = self.conn.query_tree(self.id).ok()?.reply().ok()?;
        let child = *tree.children.first()?;
        let g = self.conn.get_geometry(child).ok()?.reply().ok()?;
        let me = self.conn.get_geometry(self.id).ok()?.reply().ok()?;
        let (w, h) = (g.width as u32 + g.x.max(0) as u32, g.height as u32 + g.y.max(0) as u32);
        if (w, h) != (me.width as u32, me.height as u32) && w > 1 && h > 1 { self.resize(w, h); }
        Some((w, h))
    }

    /// Drains pending events. Returns false once the user closed the window.
    pub fn pump(&mut self) -> bool {
        while let Ok(Some(ev)) = self.conn.poll_for_event() {
            if let x11rb::protocol::Event::ClientMessage(m) = ev {
                if m.data.as_data32()[0] == self.wm_delete {
                    self.closed_by_user = true;
                }
            }
        }
        !self.closed_by_user
    }

    /// Reads the window's pixels back and counts distinct colours. A blank window gives 1.
    pub fn distinct_colours(&self) -> Result<usize, String> {
        let geo = self.conn.get_geometry(self.id).map_err(|e| e.to_string())?.reply().map_err(|e| e.to_string())?;
        let img = self.conn.get_image(ImageFormat::Z_PIXMAP, self.id, 0, 0, geo.width, geo.height, !0)
            .map_err(|e| e.to_string())?.reply().map_err(|e| format!("get_image: {e}"))?;
        let mut set = std::collections::HashSet::new();
        for px in img.data.chunks_exact(4).step_by(7) {
            set.insert([px[0], px[1], px[2]]);
            if set.len() > 4096 { break; }
        }
        Ok(set.len())
    }

    pub fn close(self) {
        let _ = self.conn.destroy_window(self.id);
        let _ = self.conn.flush();
    }
}
