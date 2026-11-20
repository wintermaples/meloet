//! Plugin windows, visits to them and the parameters of a plugin through the whole core,
//! with the real `meloet-plughost`: its fake plugins, and the real Surge XT. The tests that
//! put a window on the screen need an X display and are ignored by default.

#[allow(dead_code)]
#[path = "../../../meloet-plughost/tests/common/x11.rs"]
mod x11;

use std::path::Path;
use std::time::{Duration, Instant};

use meloet_model::StepKind;
use meloet_plugin::scan::{BundleRecord, BundleStamp, PluginInfo};
use meloet_plugin::{EditorKind, EffectCategory, InstrumentCategory, PluginFormat, PluginKind};
use meloet_protocol::{
    AppState, Band, InsertRef, ParamEnd, ParamKind, ParamRef, ParamValues, PluginSlotRef,
    PluginState, PluginWindowView, SelectedParams, SelectedPlugin, ServerMessage, ShelfMode,
    SoundRef, StatusMessage, StripRef,
};

use super::plug_tests::{PATIENCE, SURGE_BUNDLE, options, plughost, plugin_of, state_when};
use super::*;
use crate::plugins::hosts::wait;
use plughost::wait_for;
use x11::X;

const LEVEL: u32 = 0;
/// Long enough for news to come and be handled, when a test waits for nothing to happen.
const QUIET: Duration = Duration::from_millis(400);

fn fake_key(name: &str) -> String {
    format!("lv2:urn:meloet:fake:{name}")
}

/// A scan file that knows the fake plugins with windows, which no fake bundle lists. What
/// it says of their windows is wrong on purpose: the plugin's own word counts once loaded.
fn know_window_fakes(root: &Path) {
    let bundle = root.join("lv2/windows.lv2");
    std::fs::create_dir_all(&bundle).unwrap();
    std::fs::write(bundle.join("plugin.so"), [0u8; 16]).unwrap();
    let info = |name: &str, kind: PluginKind| PluginInfo {
        key: fake_key(name).parse().unwrap(),
        name: format!("Fake {name}"),
        vendor: "meloet".into(),
        version: "1".into(),
        kind,
        editor: EditorKind::None,
        audio_inputs: if matches!(kind, PluginKind::Instrument(_)) {
            0
        } else {
            2
        },
        audio_outputs: 2,
    };
    let synth = PluginKind::Instrument(InstrumentCategory::Synth);
    let mut cache = ScanCache::default();
    cache.put(BundleRecord {
        stamp: BundleStamp::read(&bundle).unwrap(),
        path: bundle,
        format: PluginFormat::Lv2,
        plugins: vec![
            info("window", synth),
            info("knob", synth),
            info("slowwindow", synth),
            info("notyet", PluginKind::Effect(EffectCategory::Other)),
        ],
        stopped: Vec::new(),
    });
    cache.save(&root.join("cache")).unwrap();
}

/// The `ParamValues` messages a screen got, each with the time it came at.
type Seen = Arc<Mutex<Vec<(Instant, ParamValues)>>>;

fn watch_values(core: &Core) -> Seen {
    let seen = Seen::default();
    let sink = seen.clone();
    let messages = move |message| {
        if let ServerMessage::ParamValues(values) = message {
            lock(&sink).push((Instant::now(), values));
        }
        true
    };
    core.subscribe(Box::new(messages), Box::new(|_| true));
    seen
}

/// Messages of one table are numbered from 1 without a gap and come at most 30 a second.
fn check_values(seen: &[(Instant, ParamValues)], table: u32) {
    let of_table: Vec<_> = seen.iter().filter(|(_, v)| v.table == table).collect();
    for (n, (_, values)) in of_table.iter().enumerate() {
        assert_eq!(values.seq, n as u64 + 1, "{of_table:?}");
    }
    for pair in of_table.windows(2) {
        let apart = pair[1].0.duration_since(pair[0].0);
        // Sent at least 33.3 ms apart; the sink sees them a little earlier or later.
        assert!(apart >= Duration::from_millis(28), "{apart:?} apart");
    }
}

fn selected(state: &AppState) -> Option<SelectedPlugin> {
    match &state.params.selected {
        Some(SelectedParams::Plugin(plugin)) => Some(plugin.clone()),
        _ => None,
    }
}

/// The table of the selected plugin, once the plugin has listed its parameters.
fn table_when(core: &Core, not: Option<u32>) -> u32 {
    let listed = |s: &AppState| selected(s).is_some_and(|p| p.table.is_some() && p.table != not);
    let state = state_when(core, "the plugin lists its parameters", listed);
    selected(&state).unwrap().table.unwrap()
}

impl Core {
    /// A new track with the plugin of `key`, loaded. Returns the track's id.
    fn plugin_track(&self, key: &str) -> u32 {
        let sound = SoundRef::Plugin { key: key.into() };
        self.send(Command::OpenShelf {
            mode: ShelfMode::NewTrack,
        });
        self.send(Command::ShelfSelectSound {
            sound: Some(sound.clone()),
        });
        state_when(self, "the plugin is read", |s| {
            s.shelf.selected_sound == Some(sound.clone()) && s.shelf.loading_sound.is_none()
        });
        self.send(Command::ShelfConfirm);
        self.send(Command::ToggleBand { band: Band::Shelf });
        let state = self.snapshot().state;
        let at = state.song.tracks.len() - 1;
        assert_eq!(plugin_of(&state, at), Some(PluginState::Ready));
        state.song.tracks[at].id
    }

    fn undo_count(&self) -> u32 {
        self.snapshot().state.song.undo_count
    }

    fn last_step(&self) -> Option<StepKind> {
        self.peek(|w| w.model.play().steps().last().cloned())
    }

    /// Waits until the song has `count` steps to take back.
    fn steps_when(&self, count: u32, what: &str) {
        state_when(self, what, |s| s.song.undo_count == count);
    }

    /// Waits until parameter `param` of the plugin is `value`.
    fn value_when(&self, id: SlotId, param: u32, value: f64, what: &str) {
        self.near_when(id, param, value, 1e-6, what);
    }

    /// Waits until parameter `param` of the plugin is no further than `off` from `value`.
    fn near_when(&self, id: SlotId, param: u32, value: f64, off: f64, what: &str) {
        let now = || {
            self.peek(move |w| {
                let hosts = &w.plugins.as_ref().unwrap().hosts;
                wait(|done| hosts.param(id, param, done)).unwrap().0
            })
        };
        wait_for(PATIENCE, || ((now() - value).abs() < off).then_some(()))
            .unwrap_or_else(|| panic!("{what}: it is {}", now()));
    }

    /// The person turns a parameter in the plugin's window: the plugin has the value, and
    /// meloet was not asked.
    fn turn(&self, id: SlotId, param: u32, value: f64) {
        self.peek(move |w| {
            let hosts = &w.plugins.as_ref().unwrap().hosts;
            wait(|done| hosts.set_param(id, param, value, done)).unwrap();
        });
    }

    /// As the plugin process says of a hand that took a parameter in the window. What
    /// [`Core::turn`] does comes from the host's side, and the plugin counts no hand for it.
    fn hand(&self, track: u32) {
        self.peek(move |w| {
            let place = meloet_model::PluginPlace::Instrument(meloet_model::TrackId(track));
            let tickets = w.model.plugin_tickets();
            let (_, ticket) = *tickets.iter().find(|(p, _)| *p == place).expect("loaded");
            let said = crate::state::PluginSaid::WindowTouches { touches: 1 };
            w.handle(crate::state::Input::PluginSaid { ticket, said });
        });
    }

    /// Waits until a visit to a plugin's window is under way, or until none is.
    fn visit_when(&self, under_way: bool) {
        let visiting = || self.peek(|w| w.model.play().visiting());
        wait_for(PATIENCE, || (visiting() == under_way).then_some(()))
            .unwrap_or_else(|| panic!("a visit is under way: {}", !under_way));
    }

    fn window_pid(&self, key: &str) -> u32 {
        let processes = self.processes();
        let of_key = processes.iter().find(|p| p.key.to_string() == key);
        of_key.unwrap_or_else(|| panic!("no process of {key}")).pid
    }
}

#[test]
fn a_plugins_parameters_are_listed_dragged_typed_and_taken_back_through_the_core() {
    let root = plughost::temp_dir("plug-params");
    let core = Core::start(options(&root));
    let seen = watch_values(&core);
    state_when(&core, "the scan", |s| s.library.plugins.len() == 2);
    let track = core.plugin_track(&fake_key("sine"));
    let slot = PluginSlotRef::Instrument { track };
    let id = core.track_plugin(1);

    // Selected: it has no window, so the panel comes to the front, with its table.
    core.send(Command::SelectInstrument { track });
    let table = table_when(&core, None);
    let state = core.snapshot().state;
    assert_eq!(state.workspace.layout.right_front, Some(Band::Params));
    let panel = selected(&state).unwrap();
    assert_eq!(panel.window, meloet_protocol::PluginWindowKind::None);
    assert_eq!((panel.slot, panel.count), (slot, 1));
    let rows = core.plugin_params(table).expect("the table");
    assert_eq!((rows.table, rows.seq, rows.groups.len()), (table, 0, 1));
    let level = &rows.groups[0].params[0];
    assert_eq!((level.id, level.name.as_str()), (LEVEL, "Level"));
    assert_eq!(
        (level.kind.clone(), level.value),
        (ParamKind::Smooth, Some(0.5))
    );
    assert_eq!(level.text.as_deref(), Some("50 %"));
    assert_eq!(core.plugin_params(table + 1), None);

    // Dragged for a third of a second and let go: heard all along, one step, and the screen
    // got the values with the plugin's texts, no more than 30 times a second.
    let steps = core.undo_count();
    let set = |table: u32, value: f64, commit: bool| {
        core.send(Command::SetPluginParam {
            slot,
            table,
            param: LEVEL,
            value,
            commit,
        });
    };
    for n in 1..=100 {
        set(table, 0.5 + f64::from(n) * 0.002, false);
        std::thread::sleep(Duration::from_millis(3));
    }
    core.value_when(id, LEVEL, 0.7, "the drag is heard");
    assert_eq!(core.undo_count(), steps);
    set(table, 0.8, true);
    core.steps_when(steps + 1, "the drag is a step");
    assert!(matches!(
        core.last_step(),
        Some(StepKind::SetPluginParam { param: LEVEL, .. })
    ));
    core.value_when(id, LEVEL, 0.8, "let go");
    let last = |seen: &Seen| lock(seen).last().map(|(_, v)| v.values.clone());
    let came = wait_for(PATIENCE, || {
        last(&seen).filter(|values| values.iter().any(|v| v.value == Some(0.8)))
    });
    let came = came.expect("the last value comes");
    assert_eq!(came[0].text.as_deref(), Some("80 %"));
    let messages = lock(&seen).clone();
    assert!((3..=16).contains(&messages.len()), "{}", messages.len());
    check_values(&messages, table);

    // Undo and redo: the plugin gets its whole state back, and the screen a new table.
    core.send(Command::Undo);
    core.value_when(id, LEVEL, 0.5, "taken back");
    let again = table_when(&core, Some(table));
    let rows = core.plugin_params(again).expect("the new table");
    assert_eq!(rows.groups[0].params[0].value, Some(0.5));
    assert_eq!(core.processes().len(), 1, "not loaded anew");
    core.send(Command::Redo);
    core.value_when(id, LEVEL, 0.8, "made again");
    let table = table_when(&core, Some(again));

    // Esc during a drag: the value from before it.
    set(table, 0.2, false);
    core.value_when(id, LEVEL, 0.2, "dragged");
    core.send(Command::CancelMixPreview);
    core.value_when(id, LEVEL, 0.8, "given up");
    assert_eq!(core.undo_count(), steps + 1);

    // Typed: the plugin reads the text. Past the end it is put at the end and said so with
    // the plugin's own texts; what is no value changes nothing and is said too.
    let typed = |text: &str| {
        let param = ParamRef::Plugin {
            slot,
            table,
            param: LEVEL,
        };
        let text = text.to_owned();
        core.send(Command::TypeParam { param, text });
    };
    typed("30 %");
    core.steps_when(steps + 2, "the typed value is a step");
    core.value_when(id, LEVEL, 0.3, "typed");
    typed("140");
    let clamped = StatusMessage::ParamClamped {
        to: ParamEnd::Plugin {
            name: "Level".into(),
            value: "100 %".into(),
            range: "0 %〜100 %".into(),
        },
    };
    state_when(&core, "said: put at the end", |s| {
        s.status.message == Some(clamped.clone())
    });
    core.steps_when(steps + 3, "the end is a step");
    core.value_when(id, LEVEL, 1.0, "at the end");
    typed("much");
    let state = state_when(&core, "said: not a value", |s| {
        matches!(s.status.message, Some(StatusMessage::ParamRejected { .. }))
    });
    let param = ParamRef::Plugin {
        slot,
        table,
        param: LEVEL,
    };
    let kept = "100 %".into();
    let rejected = StatusMessage::ParamRejected { param, kept };
    assert_eq!(state.status.message, Some(rejected));
    assert_eq!(core.undo_count(), steps + 3);

    // A row is chosen by the plugin's number for it.
    core.send(Command::SelectParam { param: Some(LEVEL) });
    let state = core.snapshot().state;
    assert_eq!(selected(&state).unwrap().row, Some(LEVEL));
    drop(core);
    let _ = std::fs::remove_dir_all(root);
}

fn window_row(slot: PluginSlotRef, name: &str) -> PluginWindowView {
    PluginWindowView {
        slot: Some(slot),
        name: name.into(),
        width: 320,
        height: 200,
        larger_than_screen: false,
    }
}

#[test]
#[ignore = "needs an X display"]
fn windows_open_are_listed_and_a_visit_is_one_step_through_the_core() {
    let root = plughost::temp_dir("plug-windows");
    know_window_fakes(&root);
    let core = Core::start(options(&root));
    let seen = watch_values(&core);
    let x = X::open();
    state_when(&core, "the scan", |s| s.library.plugins.len() == 6);

    // A plugin with a window: once loaded, it says so itself. Selected, its window opens
    // and the panel stays as it is.
    let track = core.plugin_track(&fake_key("window"));
    let slot = PluginSlotRef::Instrument { track };
    let id = core.track_plugin(1);
    core.send(Command::SelectInstrument { track });
    let state = state_when(&core, "the window is up", |s| !s.windows.list.is_empty());
    assert_eq!(state.windows.list, [window_row(slot, "Fake window")]);
    let one = StatusMessage::PluginWindowOpen {
        name: "Fake window".into(),
    };
    assert_eq!(state.status.message, Some(one));
    assert_eq!(
        selected(&state).unwrap().window,
        meloet_protocol::PluginWindowKind::Opens
    );
    assert_ne!(state.workspace.layout.right_front, Some(Band::Params));
    let pid = core.window_pid(&fake_key("window"));
    let window = x.wait(pid, "Fake window", true).expect("on the screen");
    let table = table_when(&core, None);
    assert_eq!(selected(&core.snapshot().state).unwrap().count, 3);

    // The desktop may have given the new window the keyboard: out of it first.
    std::thread::sleep(QUIET);
    x.unfocus();
    core.visit_when(false);
    std::thread::sleep(QUIET);
    let steps = core.undo_count();

    // In and out with nothing changed: no step.
    x.focus(window);
    core.visit_when(true);
    x.unfocus();
    core.visit_when(false);
    std::thread::sleep(QUIET);
    assert_eq!(core.undo_count(), steps);

    // In, two things changed, out: one step. Undo and redo put the plugin back and forth,
    // and the window stays.
    x.focus(window);
    core.visit_when(true);
    core.turn(id, LEVEL, 0.3);
    core.turn(id, LEVEL, 0.2);
    assert_eq!(core.undo_count(), steps, "not before the person is back");
    x.unfocus();
    core.steps_when(steps + 1, "the visit is a step");
    assert!(matches!(
        core.last_step(),
        Some(StepKind::PluginWindow { .. })
    ));
    core.send(Command::Undo);
    core.value_when(id, LEVEL, 0.5, "before the visit");
    core.send(Command::Redo);
    core.value_when(id, LEVEL, 0.2, "after the visit");
    assert_eq!(core.snapshot().state.windows.list.len(), 1);
    assert!(x.on_screen(window));
    assert_ne!(table_when(&core, Some(table)), table);
    std::thread::sleep(QUIET);
    let table = table_when(&core, None);

    // The keyboard is in the window while a row of the panel is let go: the window's
    // changes until then are a step, then the panel's.
    x.focus(window);
    core.visit_when(true);
    core.turn(id, LEVEL, 0.6);
    core.send(Command::SetPluginParam {
        slot,
        table,
        param: LEVEL,
        value: 0.9,
        commit: true,
    });
    core.steps_when(steps + 3, "two steps");
    let made = core.peek(|w| w.model.play().steps());
    let kinds: Vec<bool> = (made[made.len() - 2..].iter())
        .map(|step| matches!(step, StepKind::PluginWindow { .. }))
        .collect();
    assert_eq!(kinds, [true, false], "{made:?}");

    // Closed with its own button while the person is in it: gone from the list.
    x.ask_to_close(window);
    state_when(&core, "closed", |s| s.windows.list.is_empty());
    assert_eq!(x.wait(pid, "Fake window", false), None);
    std::thread::sleep(QUIET);
    assert_eq!(core.undo_count(), steps + 3, "nothing was changed since");

    // A knob turned in a window: the values come to the screen with their texts, at most
    // 30 a second, the last one for sure.
    let knob_track = core.plugin_track(&fake_key("knob"));
    let knob_slot = PluginSlotRef::Instrument { track: knob_track };
    lock(&seen).clear();
    core.send(Command::SelectInstrument { track: knob_track });
    let state = state_when(&core, "the knob's window", |s| !s.windows.list.is_empty());
    assert_eq!(state.windows.list, [window_row(knob_slot, "Fake knob")]);
    let knob = core.track_plugin(2);
    core.value_when(knob, LEVEL, 0.9, "the knob is turned up");
    let last = |seen: &Seen| lock(seen).last().map(|(_, v)| v.values.clone());
    let came = wait_for(PATIENCE, || {
        last(&seen).filter(|values| values.iter().any(|v| v.text.as_deref() == Some("90 %")))
    });
    assert!(came.is_some(), "{:?}", lock(&seen));
    let knob_table = selected(&core.snapshot().state).unwrap().table.unwrap();
    check_values(&lock(&seen), knob_table);

    // A second window: both are listed in the order they came up, and said.
    core.send(Command::OpenPluginWindow { slot: Some(slot) });
    let state = state_when(&core, "two windows", |s| s.windows.list.len() == 2);
    let rows = [
        window_row(knob_slot, "Fake knob"),
        window_row(slot, "Fake window"),
    ];
    assert_eq!(state.windows.list, rows);
    let two = StatusMessage::PluginWindowsOpen { count: 2 };
    assert_eq!(state.status.message, Some(two.clone()));
    core.send(Command::RaisePluginWindows);

    // The track is deleted: its window is gone from the list and from the screen.
    core.send(Command::DeleteTrack { track });
    let state = state_when(&core, "one window", |s| s.windows.list.len() == 1);
    assert_eq!(state.windows.list[0].slot, Some(knob_slot));
    assert_eq!(x.wait(pid, "Fake window", false), None);

    // A window that takes 0.4 s to open: said after 0.2 s, until it is up.
    let slow_track = core.plugin_track(&fake_key("slowwindow"));
    let slow = PluginSlotRef::Instrument { track: slow_track };
    let asked = Instant::now();
    core.send(Command::SelectInstrument { track: slow_track });
    let opening = StatusMessage::PluginWindowOpening {
        name: "Fake slowwindow".into(),
        slot: Some(slow),
    };
    state_when(&core, "said: opening", |s| {
        s.status.message == Some(opening.clone())
    });
    let said_after = asked.elapsed();
    assert!(
        (Duration::from_millis(200)..Duration::from_millis(450)).contains(&said_after),
        "said after {said_after:?}"
    );
    let state = state_when(&core, "the slow window", |s| s.windows.list.len() == 2);
    assert_eq!(state.status.message, Some(two));

    // One whose window meloet does not open: the panel comes instead, and it is said.
    let strip = StripRef::Track { id: slow_track };
    core.send(Command::OpenShelf {
        mode: ShelfMode::InsertEffect { strip, slot: 0 },
    });
    core.send(Command::ShelfChooseEffect {
        effect: Some(meloet_protocol::EffectRef::Plugin {
            key: fake_key("notyet"),
        }),
    });
    core.send(Command::ShelfConfirm);
    let insert = InsertRef { strip, slot: 0 };
    let not_yet = PluginSlotRef::Insert { insert };
    state_when(&core, "the effect is read", |s| {
        let kind = meloet_protocol::PluginWindowKind::NotYet;
        selected(s).is_some_and(|p| p.slot == not_yet && p.window == kind && p.table.is_some())
    });
    core.send(Command::SelectInsert {
        insert: Some(insert),
    });
    let state = core.snapshot().state;
    assert_eq!(state.workspace.layout.right_front, Some(Band::Params));
    assert!(matches!(
        state.status.message,
        Some(StatusMessage::PluginWindowNotYet { .. })
    ));
    assert_eq!(state.windows.list.len(), 2);

    // The process of a plugin with an open window dies: its row goes without any news.
    let knob_pid = core.window_pid(&fake_key("knob"));
    super::plug_tests::kill(knob_pid);
    let state = state_when(&core, "the knob's window is gone", |s| {
        s.windows.list.len() == 1
    });
    assert_eq!(state.windows.list[0].slot, Some(slow));
    drop(core);
    let _ = std::fs::remove_dir_all(root);
}

/// The real Surge XT (VST3): its window opens, what is changed while the person is in it is
/// one step, and undo gives the plugin back what it had before.
#[test]
#[ignore = "needs an X display and Surge XT (VST3)"]
fn surge_xt_a_visit_to_its_window_is_one_step_and_undo_puts_it_back() {
    assert!(Path::new(SURGE_BUNDLE).is_dir(), "no {SURGE_BUNDLE}");
    let root = plughost::temp_dir("plug-surge-window");
    std::fs::create_dir_all(root.join("vst3")).unwrap();
    std::os::unix::fs::symlink(SURGE_BUNDLE, root.join("vst3/Surge XT.vst3")).unwrap();
    let core = Core::start(CoreOptions {
        config_dir: Some(root.join("config")),
        songs_dir: Some(root.join("songs")),
        plugins: Some(PluginOptions {
            host_exe: plughost::exe(),
            cache_dir: Some(root.join("cache")),
            usual_places: vec![root.join("vst3")],
            scan_timeout: Duration::from_secs(30),
            child_env: Vec::new(),
            wait_for_screen: false,
        }),
        ..CoreOptions::offline()
    });
    let x = X::open();
    let state = state_when(&core, "the scan", |s| {
        s.library.plugins.iter().any(|row| row.name == "Surge XT")
    });
    let rows = &state.library.plugins;
    let key = rows
        .iter()
        .find(|row| row.name == "Surge XT")
        .unwrap()
        .key
        .clone();
    assert!(key.starts_with("vst3:"), "{key}");
    let track = core.plugin_track(&key);
    let slot = PluginSlotRef::Instrument { track };
    let id = core.track_plugin(1);

    // The name of the instrument is pressed: selected, and the window opens.
    let asked = Instant::now();
    core.send(Command::SelectInstrument { track });
    let state = state_when(&core, "Surge XT's window", |s| !s.windows.list.is_empty());
    let row = &state.windows.list[0];
    eprintln!("the window is up after {:?}: {row:?}", asked.elapsed());
    assert_eq!((row.slot, row.name.as_str()), (Some(slot), "Surge XT"));
    assert!(row.width > 300 && row.height > 200);
    let said = match row.larger_than_screen {
        true => matches!(
            state.status.message,
            Some(StatusMessage::PluginWindowLarge { .. })
        ),
        false => {
            let one = StatusMessage::PluginWindowOpen {
                name: "Surge XT".into(),
            };
            state.status.message == Some(one)
        }
    };
    assert!(said, "{:?}", state.status.message);
    let window = x
        .wait(core.window_pid(&key), "Surge XT", true)
        .expect("on the screen");

    // Its parameters: thousands, in groups, fetched once.
    let table = table_when(&core, None);
    let panel = selected(&core.snapshot().state).unwrap();
    let fetched = Instant::now();
    let rows = core.plugin_params(table).expect("the table");
    let all = || rows.groups.iter().flat_map(|g| &g.params);
    eprintln!(
        "{} parameters in {} groups, fetched in {:?}",
        panel.count,
        rows.groups.len(),
        fetched.elapsed()
    );
    assert_eq!(all().count(), panel.count as usize);
    assert!(panel.count > 500 && rows.groups.len() > 3);
    let octave = all().find(|p| p.name == "A Octave").expect("A Octave");
    let (octave, before) = (octave.id, octave.value.expect("its value"));

    std::thread::sleep(QUIET);
    x.unfocus();
    core.visit_when(false);
    std::thread::sleep(QUIET);
    let steps = core.undo_count();

    // In and out with nothing changed: Surge XT says the same state twice, so no step.
    x.focus(window);
    core.visit_when(true);
    x.unfocus();
    core.visit_when(false);
    std::thread::sleep(QUIET);
    assert_eq!(core.undo_count(), steps, "a visit that changed nothing");

    // In, the octave of oscillator A turned up, out: one step.
    x.focus(window);
    core.visit_when(true);
    core.turn(id, octave, 1.0);
    x.unfocus();
    core.steps_when(steps + 1, "the visit is a step");
    assert!(matches!(
        core.last_step(),
        Some(StepKind::PluginWindow { .. })
    ));
    // Undo: as before the visit, in the same process and with the window still open.
    let pid = core.window_pid(&key);
    // Surge XT gives a stepped parameter back a little inside its step (0.995 for 1).
    core.send(Command::Undo);
    core.near_when(id, octave, before, 0.01, "before the visit");
    core.send(Command::Redo);
    core.near_when(id, octave, 1.0, 0.01, "after the visit");
    assert_eq!(core.window_pid(&key), pid, "not loaded anew");
    assert_eq!(core.snapshot().state.windows.list.len(), 1);
    assert!(x.on_screen(window));
    // The screen is given a new table with the values as they are now.
    let octave_now = || {
        let table = selected(&core.snapshot().state)?
            .table
            .filter(|t| *t != table)?;
        let rows = core.plugin_params(table)?;
        let mut all = rows.groups.iter().flat_map(|g| &g.params);
        all.find(|p| p.id == octave)?
            .value
            .filter(|value| *value > 0.99)
    };
    assert!(wait_for(PATIENCE, octave_now).is_some(), "the new table");

    // The track is deleted: the window goes with its plugin.
    core.send(Command::DeleteTrack { track });
    state_when(&core, "no window", |s| s.windows.list.is_empty());
    assert_eq!(x.wait(pid, "Surge XT", false), None);
    drop(core);
    let _ = std::fs::remove_dir_all(root);
}

/// The real Surge XT (VST3): a jump through the history over two visits to its window and
/// steps of other things gives the plugin the state of the row, once, in the same process.
#[test]
#[ignore = "needs an X display and Surge XT (VST3)"]
fn surge_xt_a_jump_over_visits_to_its_window_puts_the_state_of_the_row_back() {
    assert!(Path::new(SURGE_BUNDLE).is_dir(), "no {SURGE_BUNDLE}");
    let root = plughost::temp_dir("plug-surge-jump");
    std::fs::create_dir_all(root.join("vst3")).unwrap();
    std::os::unix::fs::symlink(SURGE_BUNDLE, root.join("vst3/Surge XT.vst3")).unwrap();
    let core = Core::start(CoreOptions {
        config_dir: Some(root.join("config")),
        songs_dir: Some(root.join("songs")),
        plugins: Some(PluginOptions {
            host_exe: plughost::exe(),
            cache_dir: Some(root.join("cache")),
            usual_places: vec![root.join("vst3")],
            scan_timeout: Duration::from_secs(30),
            child_env: Vec::new(),
            wait_for_screen: false,
        }),
        ..CoreOptions::offline()
    });
    let x = X::open();
    let state = state_when(&core, "the scan", |s| {
        s.library.plugins.iter().any(|row| row.name == "Surge XT")
    });
    let rows = &state.library.plugins;
    let key = rows.iter().find(|row| row.name == "Surge XT").unwrap();
    let key = key.key.clone();
    let track = core.plugin_track(&key);
    let id = core.track_plugin(1);
    core.send(Command::SelectInstrument { track });
    state_when(&core, "Surge XT's window", |s| !s.windows.list.is_empty());
    let pid = core.window_pid(&key);
    let window = x.wait(pid, "Surge XT", true).expect("on the screen");
    let table = table_when(&core, None);
    let rows = core.plugin_params(table).expect("the table");
    let all = || rows.groups.iter().flat_map(|g| &g.params);
    let octave = all().find(|p| p.name == "A Octave").expect("A Octave");
    let (octave, before) = (octave.id, octave.value.expect("its value"));
    std::thread::sleep(QUIET);
    x.unfocus();
    core.visit_when(false);
    std::thread::sleep(QUIET);

    // Steps of other things, a visit that turns the octave up, more steps, a visit that
    // turns it down, more steps.
    let visit = |value: f64, what: &str| {
        let steps = core.undo_count();
        x.focus(window);
        core.visit_when(true);
        core.turn(id, octave, value);
        core.hand(track);
        x.unfocus();
        core.steps_when(steps + 1, what);
        assert!(matches!(
            core.last_step(),
            Some(StepKind::PluginWindow { .. })
        ));
        steps + 1
    };
    let other = |n: u32| {
        let steps = core.undo_count();
        core.send(Command::SetTempo {
            bpm: 100.0 + f64::from(n),
        });
        let strip = StripRef::Track { id: track };
        let db = Some(-(n as f32));
        core.send(Command::SetGain {
            strip,
            db,
            commit: true,
        });
        core.steps_when(steps + 2, "two steps of other things");
    };
    let start = core.undo_count();
    other(1);
    let up = visit(1.0, "the first visit is a step");
    other(2);
    let down = visit(0.0, "the second visit is a step");
    other(3);
    let end = core.undo_count();
    assert_eq!(end, start + 8);
    core.near_when(id, octave, 0.0, 0.01, "after the second visit");

    let jump = |index: u32, octave_then: f64, what: &str| {
        let row = core.history_rows(index, index).remove(0);
        let was = core.undo_count();
        let asked = Instant::now();
        core.send(Command::HistoryJump {
            index,
            step: row.step,
        });
        core.steps_when(index, what);
        core.near_when(id, octave, octave_then, 0.01, what);
        eprintln!(
            "{what}: {} steps, the plugin has the state after {:?}",
            was.abs_diff(index),
            asked.elapsed()
        );
        let said = core.snapshot().state.status.message;
        assert!(
            matches!(said, Some(StatusMessage::HistoryJumped { steps, .. }) if steps == was.abs_diff(index)),
            "{what}: {said:?}"
        );
        assert_eq!(core.window_pid(&key), pid, "{what}: not loaded anew");
        assert_eq!(core.snapshot().state.windows.list.len(), 1, "{what}");
        assert!(x.on_screen(window), "{what}");
    };
    // Back over both visits: as it started. Forward over both: as the second left it.
    jump(start + 1, before, "back over both visits");
    jump(end, 0.0, "forward over both visits");
    // Between the visits, from either side.
    jump(up + 1, 1.0, "back to between the visits");
    jump(start + 2, before, "back before the first visit");
    jump(down - 1, 1.0, "forward to between the visits");
    // Over steps that are not the plugin's: it is given nothing.
    jump(up, 1.0, "back over steps of other things");
    jump(down, 0.0, "onto the second visit");
    jump(end, 0.0, "to the newest step");
    std::thread::sleep(QUIET);
    assert_eq!(core.undo_count(), end, "no visit was made of the jumps");

    core.send(Command::DeleteTrack { track });
    state_when(&core, "no window", |s| s.windows.list.is_empty());
    drop(core);
    let _ = std::fs::remove_dir_all(root);
}
