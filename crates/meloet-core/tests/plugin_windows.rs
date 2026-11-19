//! Plugin windows and parameter lists through `Hosts`, with the real `meloet-plughost` and
//! its fake plugins. The tests that put a window on the screen need an X display and are
//! ignored by default (`scripts/check-all.sh --devices`).

mod plughost;

use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use meloet_core::plugins::hosts::{
    DEFAULT_GEOMETRY, EditorKind, EditorOpen, HostError, HostNews, HostOptions, Hosts, LoadSpec,
    Loaded, ParamNow, ParamShape, wait,
};
use meloet_engine::{Engine, EngineConfig, EngineParts};
use meloet_plugin::PluginKey;
use meloet_plugin::wire::Notice;

const PATIENCE: Duration = Duration::from_secs(10);

struct Side {
    hosts: Hosts,
    news: Receiver<HostNews>,
    parts: EngineParts,
}

fn side() -> Side {
    let (tx, news) = mpsc::channel();
    let options = HostOptions {
        exe: plughost::exe(),
        env: plughost::fake_env(),
        geometry: DEFAULT_GEOMETRY,
    };
    let hosts = Hosts::start(options, move |news| {
        let _ = tx.send(news);
    });
    let parts = Engine::new(EngineConfig {
        sample_rate: 48_000,
        max_frames: 256,
        max_tracks: 2,
    });
    Side { hosts, news, parts }
}

impl Side {
    fn load(&self, fake: &str) -> Loaded {
        let spec = LoadSpec {
            key: PluginKey::lv2(&format!("urn:meloet:fake:{fake}")).unwrap(),
            bundle: "/nowhere/fake.lv2".into(),
            sample_rate: 48_000,
        };
        let block = self.parts.handle.plugin_block().clone();
        wait(|done| self.hosts.load(spec, block, done)).unwrap()
    }

    /// The next news that `wanted` takes; everything before it is dropped.
    fn news<T>(&self, mut wanted: impl FnMut(HostNews) -> Option<T>) -> T {
        let until = Instant::now() + PATIENCE;
        loop {
            let left = until.saturating_duration_since(Instant::now());
            let news = self.news.recv_timeout(left).expect("the news in time");
            if let Some(found) = wanted(news) {
                return found;
            }
        }
    }
}

#[test]
fn parameters_come_with_groups_steps_values_and_texts() {
    let side = side();
    let window = side.load("window");
    assert_eq!(window.plugin.editor, EditorKind::X11);
    assert_eq!(side.load("notyet").plugin.editor, EditorKind::Unsupported);
    assert_eq!(side.load("sine").plugin.editor, EditorKind::None);

    let id = window.id;
    let params = wait(|done| side.hosts.params(id, done)).unwrap();
    let rows: Vec<_> = (params.iter())
        .map(|p| (p.name.as_str(), p.group.as_deref(), p.shape, p.value))
        .collect();
    assert_eq!(
        rows,
        [
            ("Level", None, ParamShape::Continuous, Some(0.5)),
            (
                "Mode",
                Some("Shape"),
                ParamShape::Stepped { steps: 3 },
                Some(0.0)
            ),
            ("Loud", Some("Shape"), ParamShape::Toggle, Some(0.0)),
        ]
    );
    assert_eq!(params[1].labels, ["Soft", "Hard", "Odd"]);
    assert_eq!(params[0].text.as_deref(), Some("50 %"));

    // Setting answers with the value as the plugin took it and its text.
    let now = wait(|done| side.hosts.set_param(id, 1, 0.9, done)).unwrap();
    let odd = ParamNow {
        id: 1,
        value: Some(1.0),
        text: Some("Odd".into()),
    };
    assert_eq!(now, odd);

    let ends = wait(|done| side.hosts.texts(id, 0, vec![0.0, 1.0], done)).unwrap();
    assert_eq!(ends, [Some("0 %".to_string()), Some("100 %".to_string())]);
    let read = |text: &str| wait(|done| side.hosts.read_text(id, 0, text.into(), done)).unwrap();
    let typed = read("140").expect("a number");
    assert_eq!((typed.value, typed.clamped), (1.0, true));
    assert_eq!(
        (
            typed.text.as_deref(),
            typed.low.as_deref(),
            typed.high.as_deref()
        ),
        (Some("100 %"), Some("0 %"), Some("100 %"))
    );
    let typed = read("30 %").expect("a number");
    assert_eq!((typed.value, typed.clamped), (0.3, false));
    assert_eq!(read("much"), None);
    // Reading set nothing.
    assert_eq!(wait(|done| side.hosts.param(id, 0, done)).unwrap().0, 0.5);
}

#[test]
fn a_window_that_cannot_be_made_is_news_without_another_ask() {
    let side = side();
    let sine = side.load("sine");
    let open = wait(|done| side.hosts.open_window(sine.id, "Sine".into(), done));
    assert_eq!(open, Ok(EditorOpen::Opening));
    // Nothing more is asked: the news comes by itself.
    let failed = side.news(|news| match news {
        HostNews::WindowFailed { slot, .. } => Some(slot),
        _ => None,
    });
    assert_eq!(failed, sine.id);
    assert_eq!(wait(|done| side.hosts.close_window(sine.id, done)), Ok(()));
    // A plugin that is gone has no window to open.
    wait(|done| side.hosts.unload(sine.id, done)).unwrap();
    let open = wait(|done| side.hosts.open_window(sine.id, "Sine".into(), done));
    assert_eq!(open, Err(HostError::UnknownSlot));
}

#[test]
#[ignore = "needs an X display"]
fn a_window_opens_and_what_is_turned_in_it_is_news_and_is_kept_for_a_reload() {
    let side = side();
    let knob = side.load("knob");
    let id = knob.id;
    let asked = Instant::now();
    let open = wait(|done| side.hosts.open_window(id, "Fake knob".into(), done));
    assert_eq!(open, Ok(EditorOpen::Opening));
    assert!(asked.elapsed() < Duration::from_millis(200));

    let mut seen = Vec::new();
    side.news(|news| {
        let end = matches!(
            &news,
            HostNews::Notice {
                notice: Notice::ParamEnd { .. },
                ..
            }
        );
        seen.push(news);
        end.then_some(())
    });
    let opened = HostNews::WindowOpened {
        slot: id,
        width: 320,
        height: 200,
        larger_than_screen: false,
    };
    assert!(seen.contains(&opened), "{seen:?}");
    let moves: Vec<(f64, &str)> = (seen.iter())
        .filter_map(|news| match news {
            HostNews::Notice {
                slot,
                notice:
                    Notice::ParamChanged {
                        id: 0,
                        value,
                        text: Some(text),
                        ..
                    },
            } if *slot == id => Some((*value, text.as_str())),
            _ => None,
        })
        .collect();
    assert!((2..40).contains(&moves.len()), "{moves:?}");
    let last = moves.last().unwrap();
    assert!((last.0 - 0.9).abs() < 1e-9 && last.1 == "90 %", "{moves:?}");

    // Asked for again it is the same window; closed, it is gone without news.
    let open = wait(|done| side.hosts.open_window(id, "Fake knob".into(), done));
    assert_eq!(open, Ok(EditorOpen::Raised));
    assert_eq!(wait(|done| side.hosts.close_window(id, done)), Ok(()));

    // The process dies: what was turned in the window comes back with the reload.
    let pid = side.hosts.processes()[0].pid;
    std::process::Command::new("kill")
        .args(["-KILL", &pid.to_string()])
        .status()
        .unwrap();
    side.news(|news| matches!(news, HostNews::Stopped { .. }).then_some(()));
    let open = wait(|done| side.hosts.open_window(id, "Fake knob".into(), done));
    assert_eq!(open, Err(HostError::Gone));
    let block = side.parts.handle.plugin_block().clone();
    let again = wait(|done| side.hosts.reload(id, block, done)).unwrap();
    assert_eq!(again.id, id);
    let level = wait(|done| side.hosts.param(id, 0, done)).unwrap();
    assert!((level.0 - 0.9).abs() < 1e-9, "{level:?}");
}
