//! The AI link: a socket inside the PC that AI apps reach through `meloet-mcp`, and the MCP
//! tools behind it (docs/architecture.md 24-11). It runs on a thread of its own with a
//! one-thread tokio runtime, and reaches the song only through [`Meloet`].

mod port;
mod presets;
mod scratch;
mod smf;
mod song;
pub mod surge;
mod tools;
mod trackref;

use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use std::thread::JoinHandle;
use std::time::Duration;

use meloet_core::Core;
use meloet_core::ai::{
    AiAnswer, AiAsk, AiLevels, AiMeasure, AiRefusal, AiRender, AiRendered, AiSong,
};
use meloet_protocol::AiPort;
use rmcp::ServiceExt;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub use port::{DIR, SOCKET};
pub use presets::PresetDirs;
pub use tools::refusal_text;

/// Bars a position may name.
pub const MAX_BAR: u64 = 9999;
/// No note starts past bar [`MAX_BAR`] of 12/4 at most.
pub const MAX_TICK: u64 = MAX_BAR * 1920 * 12;
/// Connections at once; more are closed at once.
const MAX_CONNECTIONS: usize = 8;

/// What the link needs of meloet. Every call may wait for the core thread; the link makes
/// them on threads of the blocking pool.
pub trait Meloet: Send + Sync + 'static {
    /// `None` once meloet has ended.
    fn song(&self) -> Option<AiSong>;
    fn ask(&self, ask: AiAsk) -> AiAnswer;
    fn measure(
        &self,
        ask: AiMeasure,
        progress: &mut dyn FnMut(u8),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<AiLevels, AiRefusal>;
    /// Renders into files, as `measure` goes.
    fn render(
        &self,
        ask: AiRender,
        progress: &mut dyn FnMut(u8),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<AiRendered, AiRefusal>;
    /// Does not wait.
    fn port(&self, port: AiPort);
}

/// The core, held weakly so that the link does not keep it alive.
pub struct CoreLink(pub Weak<Core>);

impl Meloet for CoreLink {
    fn song(&self) -> Option<AiSong> {
        self.0.upgrade()?.ai_song()
    }

    fn ask(&self, ask: AiAsk) -> AiAnswer {
        match self.0.upgrade() {
            Some(core) => core.ai(ask),
            None => AiAnswer::Refused(AiRefusal::Closing),
        }
    }

    fn measure(
        &self,
        ask: AiMeasure,
        progress: &mut dyn FnMut(u8),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<AiLevels, AiRefusal> {
        let core = self.0.upgrade().ok_or(AiRefusal::Closing)?;
        core.ai_measure(ask, progress, cancelled)
    }

    fn render(
        &self,
        ask: AiRender,
        progress: &mut dyn FnMut(u8),
        cancelled: &dyn Fn() -> bool,
    ) -> Result<AiRendered, AiRefusal> {
        let core = self.0.upgrade().ok_or(AiRefusal::Closing)?;
        core.ai_render(ask, progress, cancelled)
    }

    fn port(&self, port: AiPort) {
        if let Some(core) = self.0.upgrade() {
            core.ai_port(port);
        }
    }
}

#[derive(Debug, Clone)]
pub struct Options {
    /// `$XDG_RUNTIME_DIR`; `None`: the socket never opens.
    pub runtime_dir: Option<PathBuf>,
    /// Who may connect. Only tests set another than this process's own.
    pub expect_uid: u32,
    /// Where the folder for rendered audio is made (`std::env::temp_dir()`).
    pub temp_dir: PathBuf,
    /// Where presets of plugins are looked for and written.
    pub presets: PresetDirs,
}

impl Options {
    pub fn from_env() -> Options {
        Options {
            runtime_dir: std::env::var_os("XDG_RUNTIME_DIR")
                .filter(|d| !d.is_empty())
                .map(PathBuf::from),
            expect_uid: port::my_uid(),
            temp_dir: std::env::temp_dir(),
            presets: PresetDirs::from_env(),
        }
    }

    /// Where the socket is (or would be).
    pub fn socket_path(&self) -> Option<PathBuf> {
        (self.runtime_dir.as_ref()).map(|d| d.join(DIR).join(SOCKET))
    }
}

enum Order {
    Switch(bool),
    Stop,
}

/// The link's thread. Dropping it closes the socket and ends the thread.
pub struct AiLink {
    orders: mpsc::UnboundedSender<Order>,
    thread: Option<JoinHandle<()>>,
}

impl AiLink {
    pub fn start(meloet: Arc<dyn Meloet>, options: Options) -> std::io::Result<AiLink> {
        let (orders, rx) = mpsc::unbounded_channel();
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;
        let thread = std::thread::Builder::new()
            .name("meloet-ai".into())
            .spawn(move || {
                runtime.block_on(run(meloet, options, rx));
                // A call still waiting for the core is not waited for long.
                runtime.shutdown_timeout(Duration::from_millis(500));
            })?;
        Ok(AiLink {
            orders,
            thread: Some(thread),
        })
    }

    /// The switch 「AI とつなぐ」. Does not wait.
    pub fn switch(&self, on: bool) {
        let _ = self.orders.send(Order::Switch(on));
    }
}

impl Drop for AiLink {
    fn drop(&mut self) {
        let _ = self.orders.send(Order::Stop);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The socket while the switch is on.
struct Open {
    bound: port::Bound,
    listener: tokio::net::UnixListener,
    /// Ends every connection made through this socket.
    session: CancellationToken,
}

impl Open {
    fn close(self) {
        self.session.cancel();
        self.bound.remove();
    }
}

fn open(options: &Options) -> Option<Open> {
    let (listener, bound) = match port::open(options.runtime_dir.as_deref(), port::my_uid()) {
        Ok(opened) => opened,
        Err(e) => {
            eprintln!("meloet: the AI link does not open: {e}");
            return None;
        }
    };
    let listener = match tokio::net::UnixListener::from_std(listener) {
        Ok(listener) => listener,
        Err(e) => {
            eprintln!("meloet: the AI link does not open: {e}");
            bound.remove();
            return None;
        }
    };
    Some(Open {
        bound,
        listener,
        session: CancellationToken::new(),
    })
}

fn port_now(open: &Option<Open>, connections: &AtomicUsize) -> AiPort {
    match open {
        None => AiPort::Closed,
        Some(_) if connections.load(Ordering::SeqCst) > 0 => AiPort::Connected,
        Some(_) => AiPort::Waiting,
    }
}

async fn run(
    meloet: Arc<dyn Meloet>,
    options: Options,
    mut orders: mpsc::UnboundedReceiver<Order>,
) {
    let scratch = Arc::new(scratch::Scratch::new(
        options.temp_dir.clone(),
        port::my_uid(),
    ));
    let tools = tools::Tools::new(meloet.clone(), scratch.clone(), options.presets.clone());
    let mut sweep = tokio::time::interval(Duration::from_secs(10 * 60));
    let connections = Arc::new(AtomicUsize::new(0));
    let (ended_tx, mut ended) = mpsc::unbounded_channel::<()>();
    let mut socket: Option<Open> = None;
    let mut told = None;
    loop {
        let now = port_now(&socket, &connections);
        if told != Some(now) {
            told = Some(now);
            meloet.port(now);
        }
        let accepting = async {
            match &socket {
                Some(open) => open.listener.accept().await,
                None => std::future::pending().await,
            }
        };
        tokio::select! {
            order = orders.recv() => match order {
                Some(Order::Switch(true)) => {
                    if socket.is_none() {
                        socket = open(&options);
                    }
                }
                Some(Order::Switch(false)) => {
                    if let Some(open) = socket.take() {
                        open.close();
                    }
                    scratch.clean(Duration::ZERO);
                }
                Some(Order::Stop) | None => break,
            },
            accepted = accepting => {
                let Ok((stream, _)) = accepted else { continue };
                let uid = stream.peer_cred().map(|c| c.uid());
                if uid.as_ref().ok() != Some(&options.expect_uid) {
                    eprintln!("meloet: the AI link refused a connection of uid {uid:?}");
                    continue;
                }
                if connections.load(Ordering::SeqCst) >= MAX_CONNECTIONS {
                    eprintln!("meloet: the AI link has {MAX_CONNECTIONS} connections already");
                    continue;
                }
                let Some(open) = &socket else { continue };
                connections.fetch_add(1, Ordering::SeqCst);
                let (tools, ct) = (tools.clone(), open.session.child_token());
                let (connections, ended_tx) = (connections.clone(), ended_tx.clone());
                tokio::spawn(async move {
                    if let Ok(running) = tools.serve_with_ct(stream, ct).await {
                        let _ = running.waiting().await;
                    }
                    connections.fetch_sub(1, Ordering::SeqCst);
                    let _ = ended_tx.send(());
                });
            }
            _ = ended.recv() => {}
            _ = sweep.tick() => scratch.clean(scratch::KEEP),
        }
    }
    if let Some(open) = socket.take() {
        open.close();
    }
    scratch.clean(Duration::ZERO);
    meloet.port(AiPort::Closed);
}
