//! The X11 windows that plugins put their own windows into (on a Wayland desktop: through
//! Xwayland), and what the person does to them from outside the plugin: closing, resizing,
//! coming in and going away with the keyboard.
//!
//! One connection for the process, made when the first window is asked for. It goes through
//! libxcb, which also finds the X server of a desktop that has no socket file. The plugins
//! have connections of their own; nothing of theirs comes through this one.
//!
//! Main thread only.

use std::os::fd::{AsRawFd, RawFd};

use x11rb::connection::Connection;
use x11rb::properties::WmSizeHints;
use x11rb::protocol::Event;
use x11rb::protocol::randr::ConnectionExt as _;
use x11rb::protocol::xproto::{
    AtomEnum, ClientMessageEvent, ConfigureWindowAux, ConnectionExt as _, CreateWindowAux,
    EventMask, NotifyDetail, PropMode, StackMode, WindowClass,
};
use x11rb::wrapper::ConnectionExt as _;
use x11rb::xcb_ffi::XCBConnection;

/// What a window is made with, before the plugin says its size.
const FIRST_SIZE: (u32, u32) = (640, 480);
/// How far up from the window that has the keyboard its owner is looked for.
const MOST_PARENTS: usize = 32;

struct Atoms {
    protocols: u32,
    delete: u32,
    net_name: u32,
    utf8: u32,
    pid: u32,
    active: u32,
}

pub struct Display {
    conn: XCBConnection,
    root: u32,
    root_size: (u32, u32),
    black: u32,
    /// The bits of a window's number that count its client's own windows; the rest names the
    /// client.
    id_mask: u32,
    atoms: Atoms,
}

/// What happened to the windows made here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowEvent {
    /// The window is on the screen.
    Shown(u32),
    /// The person asked the window to close.
    CloseAsked(u32),
    /// The window has this size now.
    Sized(u32, u32, u32),
    /// A window right inside this one (the plugin's) takes this much room now.
    ChildSized(u32, u32, u32),
    /// The keyboard came to one of the windows, or left it.
    Focus,
}

/// Where the keyboard is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusAt {
    /// In this window made here, or in something inside it.
    Window(u32),
    /// In another window, which belongs to this X client (a plugin's menu or dialog).
    Client(u32),
    Elsewhere,
}

fn text(e: impl std::fmt::Display) -> String {
    format!("x11: {e}")
}

impl Display {
    pub fn open() -> Result<Display, String> {
        let (conn, screen) = XCBConnection::connect(None).map_err(text)?;
        let setup = conn.setup();
        let screen = setup.roots.get(screen).ok_or("x11: no screen")?;
        let (root, black) = (screen.root, screen.black_pixel);
        let root_size = (
            u32::from(screen.width_in_pixels),
            u32::from(screen.height_in_pixels),
        );
        let id_mask = setup.resource_id_mask;
        let atom = |name: &[u8]| -> Result<u32, String> {
            Ok(conn
                .intern_atom(false, name)
                .map_err(text)?
                .reply()
                .map_err(text)?
                .atom)
        };
        let atoms = Atoms {
            protocols: atom(b"WM_PROTOCOLS")?,
            delete: atom(b"WM_DELETE_WINDOW")?,
            net_name: atom(b"_NET_WM_NAME")?,
            utf8: atom(b"UTF8_STRING")?,
            pid: atom(b"_NET_WM_PID")?,
            active: atom(b"_NET_ACTIVE_WINDOW")?,
        };
        Ok(Display {
            conn,
            root,
            root_size,
            black,
            id_mask,
            atoms,
        })
    }

    /// For `poll`: readable when [`Display::events`] has something.
    pub fn fd(&self) -> RawFd {
        self.conn.as_raw_fd()
    }

    /// The connection is gone (the X server ended): nothing of it works any more.
    pub fn broken(&self) -> bool {
        self.conn.has_error().is_some()
    }

    fn flush(&self) {
        let _ = self.conn.flush();
    }

    /// A window headed `title`, not on the screen yet.
    pub fn create(&self, title: &str) -> Result<u32, String> {
        let conn = &self.conn;
        let id = conn.generate_id().map_err(text)?;
        let aux = CreateWindowAux::new()
            .background_pixel(self.black)
            .event_mask(
                EventMask::STRUCTURE_NOTIFY
                    | EventMask::SUBSTRUCTURE_NOTIFY
                    | EventMask::FOCUS_CHANGE,
            );
        conn.create_window(
            x11rb::COPY_DEPTH_FROM_PARENT,
            id,
            self.root,
            0,
            0,
            FIRST_SIZE.0 as u16,
            FIRST_SIZE.1 as u16,
            0,
            WindowClass::INPUT_OUTPUT,
            x11rb::COPY_FROM_PARENT,
            &aux,
        )
        .map_err(text)?;
        let replace = PropMode::REPLACE;
        // The old name is Latin-1; what does not fit is left to the new one.
        let latin: Vec<u8> = title
            .chars()
            .map(|c| u8::try_from(u32::from(c)).unwrap_or(b'?'))
            .collect();
        let pid = std::process::id();
        let set = conn
            .change_property8(replace, id, AtomEnum::WM_NAME, AtomEnum::STRING, &latin)
            .and_then(|_| {
                conn.change_property8(
                    replace,
                    id,
                    self.atoms.net_name,
                    self.atoms.utf8,
                    title.as_bytes(),
                )
            })
            .and_then(|_| {
                conn.change_property8(
                    replace,
                    id,
                    AtomEnum::WM_CLASS,
                    AtomEnum::STRING,
                    b"meloet\0meloet\0",
                )
            })
            .and_then(|_| {
                conn.change_property32(
                    replace,
                    id,
                    self.atoms.protocols,
                    AtomEnum::ATOM,
                    &[self.atoms.delete],
                )
            })
            .and_then(|_| {
                conn.change_property32(replace, id, self.atoms.pid, AtomEnum::CARDINAL, &[pid])
            });
        if let Err(e) = set {
            self.destroy(id);
            return Err(text(e));
        }
        self.flush();
        Ok(id)
    }

    fn size(&self, id: u32, (width, height): (u32, u32), resizable: bool) {
        let size = (width.max(1) as i32, height.max(1) as i32);
        let mut hints = WmSizeHints::new();
        if !resizable {
            hints.min_size = Some(size);
            hints.max_size = Some(size);
        }
        let _ = hints.set_normal_hints(&self.conn, id);
        let aux = ConfigureWindowAux::new()
            .width(size.0 as u32)
            .height(size.1 as u32);
        let _ = self.conn.configure_window(id, &aux);
    }

    /// Gives the window its size and asks for it to be put on the screen; [`WindowEvent::Shown`]
    /// says when it is (the desktop's window manager does both in its own time).
    pub fn show(&self, id: u32, size: (u32, u32), resizable: bool) {
        self.size(id, size, resizable);
        let _ = self.conn.map_window(id);
        self.flush();
    }

    pub fn resize(&self, id: u32, size: (u32, u32), resizable: bool) {
        self.size(id, size, resizable);
        self.flush();
    }

    /// Brings the window in front of the others and gives it the keyboard, as far as the
    /// desktop lets a program do that.
    pub fn raise(&self, id: u32) {
        let conn = &self.conn;
        let _ = conn.map_window(id);
        let above = ConfigureWindowAux::new().stack_mode(StackMode::ABOVE);
        let _ = conn.configure_window(id, &above);
        // 2: asked for by the person, not by a program on its own.
        let message = ClientMessageEvent::new(32, id, self.atoms.active, [2u32, 0, 0, 0, 0]);
        let _ = conn.send_event(
            false,
            self.root,
            EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
            message,
        );
        self.flush();
    }

    pub fn destroy(&self, id: u32) {
        let _ = self.conn.destroy_window(id);
        self.flush();
    }

    /// The windows right inside `id`.
    fn children(&self, id: u32) -> Vec<u32> {
        (self.conn.query_tree(id).ok())
            .and_then(|cookie| cookie.reply().ok())
            .map_or(Vec::new(), |tree| tree.children)
    }

    /// The room the plugin's window takes inside `id`. For plugins that never say a size.
    pub fn child_size(&self, id: u32) -> Option<(u32, u32)> {
        let child = *self.children(id).first()?;
        let at = self.conn.get_geometry(child).ok()?.reply().ok()?;
        let size = (
            u32::from(at.width) + at.x.max(0) as u32,
            u32::from(at.height) + at.y.max(0) as u32,
        );
        (size.0 > 1 && size.1 > 1).then_some(size)
    }

    /// The X clients that have windows right inside `id`: the plugin's.
    pub fn clients(&self, id: u32) -> Vec<u32> {
        let mut clients: Vec<u32> = (self.children(id).iter())
            .map(|child| child & !self.id_mask)
            .collect();
        clients.dedup();
        clients
    }

    /// Whether a window of this size is larger than the screen it is on. With more than one
    /// screen: the one its top left corner is on.
    pub fn larger_than_screen(&self, id: u32, (width, height): (u32, u32)) -> bool {
        let corner = (self.conn.translate_coordinates(id, self.root, 0, 0).ok())
            .and_then(|cookie| cookie.reply().ok())
            .map_or((0, 0), |at| (i32::from(at.dst_x), i32::from(at.dst_y)));
        let monitors = (self.conn.randr_get_monitors(self.root, true).ok())
            .and_then(|cookie| cookie.reply().ok())
            .map_or(Vec::new(), |reply| reply.monitors);
        let holds = |m: &&x11rb::protocol::randr::MonitorInfo| {
            let (x, y) = (i32::from(m.x), i32::from(m.y));
            (x..x + i32::from(m.width)).contains(&corner.0)
                && (y..y + i32::from(m.height)).contains(&corner.1)
        };
        let screen = (monitors.iter().find(holds))
            .or_else(|| monitors.iter().find(|m| m.primary))
            .or_else(|| monitors.first())
            .map_or(self.root_size, |m| {
                (u32::from(m.width), u32::from(m.height))
            });
        width > screen.0 || height > screen.1
    }

    /// Everything that happened since the last call.
    pub fn events(&self, events: &mut Vec<WindowEvent>) {
        while let Ok(Some(event)) = self.conn.poll_for_event() {
            match event {
                Event::ClientMessage(message)
                    if message.type_ == self.atoms.protocols
                        && message.format == 32
                        && message.data.as_data32()[0] == self.atoms.delete =>
                {
                    events.push(WindowEvent::CloseAsked(message.window));
                }
                Event::MapNotify(shown) if shown.event == shown.window => {
                    events.push(WindowEvent::Shown(shown.window));
                }
                Event::ConfigureNotify(at) => {
                    let (width, height) = (u32::from(at.width), u32::from(at.height));
                    if at.event == at.window {
                        events.push(WindowEvent::Sized(at.window, width, height));
                    } else {
                        events.push(WindowEvent::ChildSized(
                            at.event,
                            width + at.x.max(0) as u32,
                            height + at.y.max(0) as u32,
                        ));
                    }
                }
                // The pointer's own focus news says nothing about the keyboard.
                Event::FocusIn(focus) | Event::FocusOut(focus)
                    if focus.detail != NotifyDetail::POINTER
                        && events.last() != Some(&WindowEvent::Focus) =>
                {
                    events.push(WindowEvent::Focus);
                }
                _ => {}
            }
        }
    }

    /// Where the keyboard is now, as seen from the windows `mine`.
    pub fn focus(&self, mine: &[u32]) -> FocusAt {
        let focus = (self.conn.get_input_focus().ok())
            .and_then(|cookie| cookie.reply().ok())
            .map_or(0, |reply| reply.focus);
        // 0 is "nowhere" and 1 is "wherever the pointer is".
        if focus <= 1 || focus == self.root {
            return FocusAt::Elsewhere;
        }
        let mut window = focus;
        for _ in 0..MOST_PARENTS {
            if mine.contains(&window) {
                return FocusAt::Window(window);
            }
            let parent = (self.conn.query_tree(window).ok())
                .and_then(|cookie| cookie.reply().ok())
                .map_or(0, |tree| tree.parent);
            if parent == 0 || parent == self.root {
                break;
            }
            window = parent;
        }
        FocusAt::Client(focus & !self.id_mask)
    }
}
