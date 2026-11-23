use meloet_midi::Button;
use meloet_model::Gain;
use meloet_protocol::{Command, TransportState};

use super::Heard;
use crate::play::mix::tests::three_tracks;
use crate::play::tests::{cmd, ready};
use crate::state::{Effect, Input, Model};

const BAR: u64 = 4 * meloet_model::TICKS_PER_QUARTER;

fn press(m: &mut Model, button: Button) -> Vec<Effect> {
    m.handle(Input::KeyboardButton(button))
}

fn fader(m: &mut Model, t_us: u64, value: u8) {
    m.set_now(t_us, None);
    m.handle(Input::KeyboardFader { value });
}

fn tick(m: &mut Model, t_us: u64) {
    m.set_now(t_us, None);
    m.handle(Input::Tick);
}

fn state(m: &Model) -> TransportState {
    m.view().transport.state
}

fn position(m: &Model) -> u64 {
    m.view().transport.position
}

fn selected(m: &Model) -> Option<u32> {
    m.view().song.play_target
}

fn gain_db(m: &Model, track: usize) -> Option<f32> {
    m.view().song.tracks[track].gain_db
}

fn undo_count(m: &Model) -> u32 {
    m.view().song.undo_count
}

/// A button does what its command does, on a model in the same state.
fn same_as_command(setup: fn(&mut Model), button: Button, command: Command) {
    let (a, b) = (&mut ready(), &mut ready());
    setup(a);
    setup(b);
    press(a, button);
    cmd(b, command);
    assert_eq!(a.view(), b.view(), "{button:?}");
}

#[test]
fn transport_buttons_send_the_commands_of_the_keys() {
    same_as_command(|_| {}, Button::Record, Command::Record);
    same_as_command(|_| {}, Button::Cycle, Command::ToggleLoop);
    same_as_command(|_| {}, Button::Middle, Command::ToggleLoop);
    same_as_command(
        |m| {
            cmd(m, Command::ToggleLoop);
        },
        Button::Cycle,
        Command::ToggleLoop,
    );
    same_as_command(|_| {}, Button::Play, Command::PlayStop);
}

#[test]
fn play_only_starts_and_stop_only_stops() {
    let m = &mut ready();
    assert!(press(m, Button::Stop).is_empty(), "stopped already");
    press(m, Button::Play);
    assert_eq!(state(m), TransportState::Playing);
    assert!(press(m, Button::Play).is_empty(), "playing goes on");
    assert_eq!(state(m), TransportState::Playing);
    press(m, Button::Stop);
    assert_eq!(state(m), TransportState::Stopped);

    cmd(m, Command::SetCountIn { bars: 0 });
    press(m, Button::Record);
    assert_eq!(state(m), TransportState::Recording);
    assert!(press(m, Button::Play).is_empty());
    press(m, Button::Stop);
    assert_eq!(state(m), TransportState::Stopped);
}

#[test]
fn left_and_right_go_a_bar_to_the_heads_of_bars() {
    let m = &mut ready();
    press(m, Button::Right);
    assert_eq!(position(m), BAR);
    press(m, Button::FastForward);
    assert_eq!(position(m), 2 * BAR);
    press(m, Button::Left);
    assert_eq!(position(m), BAR);
    cmd(m, Command::Locate { tick: BAR + 100 });
    press(m, Button::Rewind);
    assert_eq!(position(m), BAR, "inside a bar: to its head");
    cmd(m, Command::Locate { tick: BAR + 100 });
    press(m, Button::Right);
    assert_eq!(position(m), 2 * BAR);
    press(m, Button::Left);
    press(m, Button::Left);
    assert_eq!(position(m), 0);
    press(m, Button::Left);
    assert_eq!(position(m), 0, "not before the head of the song");
    assert_eq!(undo_count(m), 0);

    // While playing, from where the listener is.
    press(m, Button::Play);
    m.set_now(
        1_000,
        Some(Heard {
            tick: 3.5 * BAR as f64,
            state: TransportState::Playing,
            lap: 0,
        }),
    );
    press(m, Button::Right);
    assert_eq!(position(m), 4 * BAR);
}

#[test]
fn left_pressed_fast_while_playing_goes_back_a_bar_each_time() {
    let m = &mut ready();
    press(m, Button::Play);
    // The listener is in bar 5 (from 1) and still hears it as the line is sent back.
    let hear = |m: &mut Model, t_us: u64, tick: u64| {
        m.set_now(
            t_us,
            Some(Heard {
                tick: tick as f64,
                state: TransportState::Playing,
                lap: 0,
            }),
        );
    };
    hear(m, 1_000_000, 4 * BAR + 500);
    press(m, Button::Left);
    assert_eq!(position(m), 4 * BAR);
    hear(m, 1_050_000, 4 * BAR + 600);
    press(m, Button::Left);
    assert_eq!(position(m), 3 * BAR);
    hear(m, 1_100_000, 4 * BAR + 700);
    press(m, Button::Left);
    assert_eq!(position(m), 2 * BAR);
    // Once the heard line has caught up, it is read again.
    hear(m, 1_500_000, 2 * BAR + 2000);
    press(m, Button::Left);
    assert_eq!(position(m), 2 * BAR);
}

#[test]
fn up_and_down_select_the_next_track_and_stop_at_the_ends() {
    let m = &mut ready();
    three_tracks(m);
    cmd(
        m,
        Command::SelectTrack {
            track: 1,
            from_arm: false,
        },
    );
    press(m, Button::Down);
    assert_eq!(selected(m), Some(2));
    press(m, Button::Down);
    press(m, Button::Down);
    assert_eq!(selected(m), Some(3));
    press(m, Button::Up);
    press(m, Button::Up);
    press(m, Button::Up);
    assert_eq!(selected(m), Some(1));
    assert_eq!(undo_count(m), 0);
}

#[test]
fn the_fader_moves_the_selected_track_only_after_it_reached_its_level() {
    let m = &mut ready();
    assert_eq!(gain_db(m, 0), Some(0.0));
    // 0 dB is at 80 % of the fader: between 101 and 102.
    for (t, value) in [(0, 10), (10, 60), (20, 100)] {
        fader(m, t, value);
        assert!(m.play().mix_previews().is_empty(), "{value}: not reached");
    }
    fader(m, 30, 103);
    assert_eq!(m.play().mix_previews().len(), 1, "passed 0 dB: heard");
    fader(m, 40, 90);
    assert_eq!(undo_count(m), 0, "heard only while it moves");
    assert_eq!(gain_db(m, 0), Some(0.0));
    assert_eq!(m.wake_at_us(), Some(40 + 500_000));

    tick(m, 40 + 499_999);
    assert_eq!(undo_count(m), 0);
    tick(m, 40 + 500_000);
    assert_eq!(undo_count(m), 1, "one step when it rests");
    let want = Gain::from_position(90.0 / 127.0).db().map(|db| db as f32);
    assert_eq!(gain_db(m, 0), want);
    assert!(m.play().mix_previews().is_empty());
    assert_eq!(m.wake_at_us(), None);

    // It follows on from there, and another rest is another step.
    fader(m, 1_000_000, 80);
    tick(m, 1_500_000);
    assert_eq!(undo_count(m), 2);
    cmd(m, Command::Undo);
    assert_eq!(gain_db(m, 0), want);
}

#[test]
fn another_track_selected_waits_for_the_fader_again() {
    let m = &mut ready();
    three_tracks(m);
    cmd(
        m,
        Command::SelectTrack {
            track: 1,
            from_arm: false,
        },
    );
    fader(m, 0, 102);
    fader(m, 10, 60);
    press(m, Button::Down);
    fader(m, 20, 50);
    assert_eq!(undo_count(m), 1, "the level of track 1 became a step");
    fader(m, 30, 70);
    assert!(m.play().mix_previews().is_empty(), "track 2 is at 0 dB");
    fader(m, 40, 110);
    assert_eq!(m.play().mix_previews().len(), 1);
    tick(m, 40 + 500_000);
    assert_eq!(undo_count(m), 2);
    assert_ne!(gain_db(m, 1), Some(0.0));
}

#[test]
fn a_level_set_elsewhere_must_be_reached_again() {
    let m = &mut ready();
    fader(m, 0, 102);
    tick(m, 500_000);
    cmd(
        m,
        Command::SetGain {
            strip: meloet_protocol::StripRef::Track { id: 1 },
            db: Some(-24.0),
            commit: true,
        },
    );
    fader(m, 600_000, 100);
    assert!(m.play().mix_previews().is_empty(), "-24 dB is at 20 %");
    fader(m, 610_000, 20);
    assert_eq!(m.play().mix_previews().len(), 1);
}
