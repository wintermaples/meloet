//! The tests' own look at the X server: finding the window a host process made, and doing to
//! it what a person does (giving it the keyboard, asking it to close).

use std::time::{Duration, Instant};

use x11rb::connection::Connection;
use x11rb::protocol::xproto::{
    AtomEnum, BUTTON_PRESS_EVENT, BUTTON_RELEASE_EVENT, ButtonPressEvent, ClientMessageEvent,
    ConnectionExt as _, EventMask, ImageFormat, InputFocus, KeyButMask, MOTION_NOTIFY_EVENT,
    MapState, Motion, MotionNotifyEvent,
};
use x11rb::xcb_ffi::XCBConnection;

pub struct X {
    conn: XCBConnection,
    root: u32,
    pid: u32,
    name: u32,
    protocols: u32,
    delete: u32,
}

impl X {
    pub fn open() -> X {
        let (conn, screen) = XCBConnection::connect(None).expect("an X display (DISPLAY)");
        let root = conn.setup().roots[screen].root;
        let atom = |name: &[u8]| conn.intern_atom(false, name).unwrap().reply().unwrap().atom;
        X {
            pid: atom(b"_NET_WM_PID"),
            name: atom(b"_NET_WM_NAME"),
            protocols: atom(b"WM_PROTOCOLS"),
            delete: atom(b"WM_DELETE_WINDOW"),
            root,
            conn,
        }
    }

    fn children(&self, window: u32) -> Vec<u32> {
        (self.conn.query_tree(window).ok())
            .and_then(|cookie| cookie.reply().ok())
            .map_or(Vec::new(), |tree| tree.children)
    }

    fn is(&self, window: u32, pid: u32, title: &str) -> bool {
        let property = |name: u32, kind: AtomEnum| {
            (self
                .conn
                .get_property(false, window, name, kind, 0, 1024)
                .ok())
            .and_then(|cookie| cookie.reply().ok())
        };
        let of_pid = property(self.pid, AtomEnum::CARDINAL)
            .and_then(|reply| reply.value32().and_then(|mut values| values.next()))
            == Some(pid);
        of_pid
            && property(self.name, AtomEnum::ANY)
                .is_some_and(|reply| reply.value == title.as_bytes())
    }

    /// The window process `pid` made with this title, wherever the window manager put it.
    pub fn find(&self, pid: u32, title: &str) -> Option<u32> {
        let mut level = vec![self.root];
        for _ in 0..4 {
            let mut next = Vec::new();
            for window in level {
                for child in self.children(window) {
                    if self.is(child, pid, title) {
                        return Some(child);
                    }
                    next.push(child);
                }
            }
            level = next;
        }
        None
    }

    /// Waits until the window is there (`true`) or gone (`false`).
    pub fn wait(&self, pid: u32, title: &str, there: bool) -> Option<u32> {
        let until = Instant::now() + Duration::from_secs(5);
        loop {
            let found = self.find(pid, title);
            if found.is_some() == there || Instant::now() > until {
                return found;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    pub fn size(&self, window: u32) -> (u32, u32) {
        let at = self.conn.get_geometry(window).unwrap().reply().unwrap();
        (u32::from(at.width), u32::from(at.height))
    }

    pub fn on_screen(&self, window: u32) -> bool {
        (self.conn.get_window_attributes(window).ok())
            .and_then(|cookie| cookie.reply().ok())
            .is_some_and(|attributes| attributes.map_state == MapState::VIEWABLE)
    }

    /// Whether something was put inside the window (a plugin's own window).
    pub fn has_inside(&self, window: u32) -> bool {
        !self.children(window).is_empty()
    }

    /// Gives the keyboard to the window, as a click on it does.
    pub fn focus(&self, window: u32) {
        let _ = (self.conn).set_input_focus(InputFocus::POINTER_ROOT, window, x11rb::CURRENT_TIME);
        self.conn.flush().unwrap();
    }

    /// Takes the keyboard away from every window, as a click on a Wayland window does.
    pub fn unfocus(&self) {
        let _ = (self.conn).set_input_focus(InputFocus::NONE, x11rb::NONE, x11rb::CURRENT_TIME);
        self.conn.flush().unwrap();
    }

    /// Tells the plugin's own window inside `window` that the left button went down at
    /// (`x`, `y`), moved by `dy` in ten steps and came up. The real pointer is not touched:
    /// the events are made up and sent to that window alone.
    pub fn drag(&self, window: u32, x: i16, y: i16, dy: i16) {
        // The deepest window at that point is the one a real pointer would talk to.
        let (mut target, mut at) = (window, (x, y));
        while let Some(reply) = (self.conn.translate_coordinates(window, target, x, y).ok())
            .and_then(|cookie| cookie.reply().ok())
        {
            at = (reply.dst_x, reply.dst_y);
            if reply.child == x11rb::NONE {
                break;
            }
            target = reply.child;
        }
        if target == window {
            return;
        }
        let root_at = (self
            .conn
            .translate_coordinates(window, self.root, x, y)
            .ok())
        .and_then(|cookie| cookie.reply().ok())
        .map_or((0, 0), |reply| (reply.dst_x, reply.dst_y));
        let button = |response_type, moved: i16, state| ButtonPressEvent {
            response_type,
            detail: 1,
            sequence: 0,
            time: x11rb::CURRENT_TIME,
            root: self.root,
            event: target,
            child: x11rb::NONE,
            root_x: root_at.0,
            root_y: root_at.1 + moved,
            event_x: at.0,
            event_y: at.1 + moved,
            state,
            same_screen: true,
        };
        let send = |mask, event: ButtonPressEvent| {
            let _ = self.conn.send_event(true, target, mask, event);
            self.conn.flush().unwrap();
            std::thread::sleep(Duration::from_millis(15));
        };
        send(
            EventMask::BUTTON_PRESS,
            button(BUTTON_PRESS_EVENT, 0, KeyButMask::default()),
        );
        for step in 1..=10 {
            let pressed = button(MOTION_NOTIFY_EVENT, dy * step / 10, KeyButMask::BUTTON1);
            let moved = MotionNotifyEvent {
                response_type: pressed.response_type,
                detail: Motion::NORMAL,
                sequence: 0,
                time: pressed.time,
                root: pressed.root,
                event: pressed.event,
                child: pressed.child,
                root_x: pressed.root_x,
                root_y: pressed.root_y,
                event_x: pressed.event_x,
                event_y: pressed.event_y,
                state: pressed.state,
                same_screen: true,
            };
            let _ = (self.conn).send_event(
                true,
                target,
                EventMask::POINTER_MOTION | EventMask::BUTTON_MOTION,
                moved,
            );
            self.conn.flush().unwrap();
            std::thread::sleep(Duration::from_millis(15));
        }
        send(
            EventMask::BUTTON_RELEASE,
            button(BUTTON_RELEASE_EVENT, dy, KeyButMask::BUTTON1),
        );
    }

    /// Writes what the window shows to a PPM file, when a person wants to look at it.
    pub fn shot(&self, window: u32, path: &std::path::Path) {
        let (width, height) = self.size(window);
        let image = (self.conn)
            .get_image(
                ImageFormat::Z_PIXMAP,
                window,
                0,
                0,
                width as u16,
                height as u16,
                !0,
            )
            .ok()
            .and_then(|cookie| cookie.reply().ok());
        let Some(image) = image else { return };
        let mut ppm = format!("P6 {width} {height} 255\n").into_bytes();
        for pixel in image.data.as_chunks::<4>().0 {
            ppm.extend([pixel[2], pixel[1], pixel[0]]);
        }
        let _ = std::fs::write(path, ppm);
    }

    /// What the window's close button does.
    pub fn ask_to_close(&self, window: u32) {
        let message =
            ClientMessageEvent::new(32, window, self.protocols, [self.delete, 0, 0, 0, 0]);
        let _ = (self.conn).send_event(false, window, EventMask::NO_EVENT, message);
        self.conn.flush().unwrap();
    }
}
