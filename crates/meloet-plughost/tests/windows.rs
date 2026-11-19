//! Plugin windows, with the fake plugins that have one. The tests that put a window on the
//! screen need an X display (on Wayland: Xwayland) and are ignored by default:
//! `scripts/check-all.sh --devices` runs them.

mod common;

use std::time::{Duration, Instant};

use common::x11::X;
use common::{Host, block, note_on, peak};
use meloet_plugin::wire::{EditorOpen, Notice, Reply, Request};

const WAIT: Duration = Duration::from_secs(5);
/// Long enough for a notice that is on its way to arrive.
const QUIET: Duration = Duration::from_millis(250);

fn open(host: &mut Host, slot: u32, title: &str) -> EditorOpen {
    match host.ask(Request::OpenEditor {
        slot,
        title: title.into(),
    }) {
        Reply::Editor { open } => open,
        other => panic!("{other:?}"),
    }
}

fn opened(host: &mut Host, slot: u32) -> Option<Notice> {
    host.wait_for(WAIT, |notice| {
        matches!(notice, Notice::EditorOpened { slot: of, .. } | Notice::EditorFailed { slot: of, .. } if *of == slot)
    })
}

/// The tests with windows run one after the other: there is one keyboard, and the desktop
/// gives it to a window that comes up.
fn alone() -> std::sync::MutexGuard<'static, ()> {
    static ONE: std::sync::Mutex<()> = std::sync::Mutex::new(());
    ONE.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn pid(host: &Host) -> u32 {
    host.child.0.id()
}

#[test]
fn a_window_that_cannot_be_made_is_told_and_the_plugin_goes_on() {
    let mut host = Host::start();
    let (mut sine, _) = host.load(0, "sine");
    assert_eq!(open(&mut host, 0, "no window"), EditorOpen::Opening);
    assert!(matches!(
        opened(&mut host, 0),
        Some(Notice::EditorFailed { slot: 0, .. })
    ));
    // Asked again, it is tried again.
    assert_eq!(open(&mut host, 0, "no window"), EditorOpen::Opening);
    assert!(matches!(
        opened(&mut host, 0),
        Some(Notice::EditorFailed { slot: 0, .. })
    ));
    assert_eq!(host.ask(Request::CloseEditor { slot: 0 }), Reply::Ok);
    let (left, _) = block(&mut sine, &[0.0; 64], &[note_on(69)]);
    assert!(peak(&left) > 0.1);
}

#[test]
#[ignore = "needs an X display"]
fn a_window_opens_comes_to_the_front_closes_and_opens_again() {
    let _alone = alone();
    let x = X::open();
    let mut host = Host::start();
    host.load(0, "window");
    let title = "Fake ウィンドウ 1";
    assert_eq!(open(&mut host, 0, title), EditorOpen::Opening);
    let size = Notice::EditorOpened {
        slot: 0,
        width: 320,
        height: 200,
        larger_than_screen: false,
    };
    assert_eq!(opened(&mut host, 0), Some(size.clone()));
    let window = x.wait(pid(&host), title, true).expect("the window");
    assert_eq!(x.size(window), (320, 200), "{:?}", host.notices);
    assert!(x.on_screen(window));

    // Asked for again, it is the same window.
    assert_eq!(open(&mut host, 0, title), EditorOpen::Raised);
    assert_eq!(x.find(pid(&host), title), Some(window));

    assert_eq!(host.ask(Request::CloseEditor { slot: 0 }), Reply::Ok);
    assert_eq!(x.wait(pid(&host), title, false), None);
    // Closing one that is closed is nothing.
    assert_eq!(host.ask(Request::CloseEditor { slot: 0 }), Reply::Ok);

    assert_eq!(open(&mut host, 0, title), EditorOpen::Opening);
    assert_eq!(opened(&mut host, 0), Some(size));
    assert!(x.wait(pid(&host), title, true).is_some());
    // A plugin that is unloaded takes its window along.
    assert_eq!(host.ask(Request::Unload { slot: 0 }), Reply::Ok);
    assert_eq!(x.wait(pid(&host), title, false), None);

    // Nothing was said about windows that meloet closed itself.
    assert!(host.wait_for(QUIET, |_| false).is_none());
    let closed = |notice: &Notice| {
        matches!(
            notice,
            Notice::EditorClosed { .. } | Notice::EditorLeft { .. }
        )
    };
    assert!(!host.notices.iter().any(closed), "{:?}", host.notices);
}

#[test]
#[ignore = "needs an X display"]
fn the_keyboard_coming_and_going_and_the_close_button_are_told() {
    let _alone = alone();
    let x = X::open();
    let mut host = Host::start();
    let (mut sine, _) = host.load(1, "window");
    let title = "Fake window 2";
    assert_eq!(open(&mut host, 1, title), EditorOpen::Opening);
    assert!(matches!(
        opened(&mut host, 1),
        Some(Notice::EditorOpened { .. })
    ));
    let window = x.wait(pid(&host), title, true).expect("the window");
    // The desktop may have given the new window the keyboard already.
    host.wait_for(QUIET, |_| false);
    x.unfocus();
    host.wait_for(QUIET, |_| false);
    host.notices.clear();

    let entered = |n: &Notice| *n == Notice::EditorEntered { slot: 1 };
    let left = |n: &Notice| *n == Notice::EditorLeft { slot: 1 };
    x.focus(window);
    assert!(host.wait_for(WAIT, entered).is_some());
    x.unfocus();
    assert!(host.wait_for(WAIT, left).is_some());
    host.notices.clear();
    x.focus(window);
    assert!(host.wait_for(WAIT, entered).is_some());

    // Closed by the person while they are in it: left, then closed.
    host.notices.clear();
    x.ask_to_close(window);
    let closed = |n: &Notice| *n == Notice::EditorClosed { slot: 1 };
    assert!(host.wait_for(WAIT, closed).is_some());
    assert_eq!(
        host.notices,
        [
            Notice::EditorLeft { slot: 1 },
            Notice::EditorClosed { slot: 1 }
        ]
    );
    assert_eq!(x.wait(pid(&host), title, false), None);
    // The plugin is as well as before, and its window opens again.
    let (out, _) = block(&mut sine, &[0.0; 64], &[note_on(69)]);
    assert!(peak(&out) > 0.1);
    assert_eq!(open(&mut host, 1, title), EditorOpen::Opening);
    assert!(matches!(
        opened(&mut host, 1),
        Some(Notice::EditorOpened { .. })
    ));
}

#[test]
#[ignore = "needs an X display"]
fn a_knob_turned_in_the_window_reaches_the_sound_and_is_told_thirty_times_a_second() {
    let _alone = alone();
    let mut host = Host::start();
    let (mut sine, _) = host.load(0, "knob");
    let (out, _) = block(&mut sine, &[0.0; 256], &[note_on(69)]);
    assert!((peak(&out) - 0.5).abs() < 0.01);

    assert_eq!(open(&mut host, 0, "Fake knob"), EditorOpen::Opening);
    let started = Instant::now();
    let end = |n: &Notice| matches!(n, Notice::ParamEnd { slot: 0, id: 0 });
    assert!(host.wait_for(WAIT, end).is_some());
    let took = started.elapsed();

    let params: Vec<&Notice> = (host.notices.iter())
        .filter(|n| {
            matches!(
                n,
                Notice::ParamBegin { .. } | Notice::ParamChanged { .. } | Notice::ParamEnd { .. }
            )
        })
        .collect();
    assert_eq!(params[0], &Notice::ParamBegin { slot: 0, id: 0 });
    let moves = &params[1..params.len() - 1];
    // Forty moves were made; no more than 30 a second were told, and the first at once.
    let most = (took.as_secs_f64() * 30.0).ceil() as usize + 1;
    assert!(
        (2..=most).contains(&moves.len()),
        "{} of {most}",
        moves.len()
    );
    assert!(moves.len() < 40);
    let mut last = 0.5;
    for told in moves {
        let Notice::ParamChanged {
            slot: 0,
            id: 0,
            value,
            text: Some(text),
        } = told
        else {
            panic!("{told:?}");
        };
        assert!(*value > last);
        assert_eq!(*text, format!("{:.0} %", value * 100.0));
        last = *value;
    }
    // The value the knob was let go at is always told.
    assert!((last - 0.9).abs() < 1e-9);
    let (out, _) = block(&mut sine, &[0.0; 256], &[]);
    assert!((peak(&out) - 0.9).abs() < 0.01);
    assert_eq!(host.param(0, 0), (0.9, "90 %".into()));
}

#[test]
#[ignore = "needs an X display"]
fn a_slow_window_is_answered_at_once_and_the_sound_does_not_wait_for_it() {
    let _alone = alone();
    let mut host = Host::start();
    let (mut sine, _) = host.load(0, "slowwindow");
    block(&mut sine, &[0.0; 256], &[note_on(69)]);
    let asked = Instant::now();
    assert_eq!(open(&mut host, 0, "Fake slow window"), EditorOpen::Opening);
    assert!(asked.elapsed() < Duration::from_millis(200));
    // The main thread of the process is in the plugin's window now; the blocks are answered
    // all the same.
    let mut blocks = 0;
    while asked.elapsed() < Duration::from_millis(300) {
        let (out, _) = block(&mut sine, &[0.0; 256], &[]);
        assert!(peak(&out) > 0.1);
        blocks += 1;
    }
    assert!(blocks > 10, "{blocks}");
    assert!(host.notices.is_empty(), "{:?}", host.notices);
    assert!(matches!(
        opened(&mut host, 0),
        Some(Notice::EditorOpened { .. })
    ));
    assert!(asked.elapsed() >= Duration::from_millis(400));
}

/// Resident memory of a process, in KiB.
fn resident(pid: u32) -> u64 {
    let text = std::fs::read_to_string(format!("/proc/{pid}/smaps_rollup")).unwrap();
    (text.lines())
        .find_map(|line| line.strip_prefix("Rss:"))
        .and_then(|rest| rest.split_whitespace().next()?.parse().ok())
        .unwrap()
}

#[test]
#[ignore = "needs an X display"]
fn three_hundred_windows_leave_nothing_behind() {
    let _alone = alone();
    let mut host = Host::start();
    host.load(0, "window");
    let round = |host: &mut Host| {
        host.notices.clear();
        assert_eq!(open(host, 0, "Fake window 300"), EditorOpen::Opening);
        assert!(matches!(opened(host, 0), Some(Notice::EditorOpened { .. })));
        assert_eq!(host.ask(Request::CloseEditor { slot: 0 }), Reply::Ok);
    };
    for _ in 0..50 {
        round(&mut host);
    }
    let before = resident(pid(&host));
    for _ in 0..250 {
        round(&mut host);
    }
    let after = resident(pid(&host));
    eprintln!("resident after 50 windows {before} KiB, after 300 {after} KiB");
    assert!(after < before + 512, "{before} KiB became {after} KiB");
}
