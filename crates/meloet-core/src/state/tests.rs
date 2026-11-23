use crate::seen::*;
use meloet_devices::{Availability as Av, ClockSettings, Friendly as Fr, LinkKind as Link};
use meloet_protocol::{
    EffectKind, InsertView, MasterView, ParamsView, ReturnView, ShelfMode, SoundView, StandIn,
    TimelineView,
};

use super::*;

const HP: &str = "card:analog-output-headphones";
pub(super) const HP_NODE: &str = "alsa_output.card.analog-stereo";
const USB: &str = "usb:analog-output";
const USB_NODE: &str = "alsa_output.usb.analog-stereo";
const HDMI: &str = "gpu:hdmi-output-0";
const LINE: &str = "card:analog-output-lineout";

fn ep(id: &str, name: &str, av: Av, active: bool, node: Option<&str>) -> Endpoint {
    Endpoint {
        id: id.into(),
        name: name.into(),
        friendly: id.contains("headphones").then_some(Fr::Headphones),
        availability: av,
        link: Link::Plug,
        active,
        node_name: node.map(str::to_owned),
        device_name: None,
        device_description: Some(format!("{} device", id.split(':').next().unwrap())),
        channels: None,
    }
}

fn headphones(av: Av, active: bool) -> Endpoint {
    ep(HP, "アナログヘッドフォン", av, active, Some(HP_NODE))
}

fn usb() -> Endpoint {
    ep(USB, "USB スピーカー", Av::Unknown, true, Some(USB_NODE))
}

/// HDMI has no node (its profile is off); line out shares the headphones' node but its Route
/// is not the one in use. Neither can be chosen.
fn others() -> [Endpoint; 2] {
    [
        ep(HDMI, "HDMI / DisplayPort", Av::Yes, false, None),
        ep(LINE, "ライン出力", Av::Yes, false, Some(HP_NODE)),
    ]
}

fn picture(outputs: Vec<Endpoint>, force_quantum: Option<u32>) -> Input {
    Input::Devices(DevicesPicture {
        outputs,
        inputs: vec![ep(
            "card:mic",
            "マイク",
            Av::No,
            true,
            Some("alsa_input.card"),
        )],
        clock: ClockSettings {
            force_quantum,
            ..ClockSettings::default()
        },
    })
}

/// In `sort_for_choice` order: HP, USB, HDMI, line out.
fn usual() -> Input {
    let [hdmi, line] = others();
    picture(vec![hdmi, usb(), headphones(Av::Yes, true), line], None)
}

fn keyboard(n: i32) -> Keyboard {
    Keyboard {
        id: format!("kb{n}"),
        name: format!("Keystation {n}"),
        port_name: "MIDI 1".into(),
        client: 20 + n,
        port: 0,
    }
}

fn fresh() -> Model {
    Model::new(Settings::default())
}

fn remembering(output: &str) -> Model {
    Model::new(Settings {
        output: Some(output.into()),
        output_name: Some("アナログヘッドフォン".into()),
        scale: 100,
        ..Settings::default()
    })
}

fn cmd(model: &mut Model, command: Command) -> Vec<Effect> {
    model.handle(Input::Command(command))
}

fn open(node: &str) -> Effect {
    Effect::OpenOutput {
        node_name: node.into(),
    }
}

fn opened(model: &mut Model, node: &str, sample_rate: u32) {
    let effects = model.handle(Input::AudioOpened {
        node_name: node.into(),
        sample_rate,
    });
    assert!(
        effects
            .iter()
            .all(|e| matches!(e, Effect::Engine(_) | Effect::MonoListen(_))),
        "{effects:?}"
    );
}

fn status(model: &Model) -> Option<StatusMessage> {
    model.view().status.message
}

/// A model that chose the headphones through the chooser and has its stream open at 48 kHz.
pub(super) fn playing() -> Model {
    let mut m = fresh();
    m.handle(Input::KeyboardAdded(keyboard(1)));
    m.handle(usual());
    cmd(&mut m, Command::ChooserConfirm);
    opened(&mut m, HP_NODE, 48_000);
    // Takes away the notice that the output was chosen.
    cmd(&mut m, Command::SelectClip { clip: None });
    m
}

/// The status line of a stopped song with nothing selected.
fn idle() -> Option<StatusMessage> {
    Some(StatusMessage::Summary {
        loop_bars: None,
        nothing_selected: true,
        notes: None,
    })
}

fn piano() -> meloet_protocol::TrackRef {
    meloet_protocol::TrackRef {
        number: 1,
        name: "ピアノ".into(),
    }
}

fn new_song() -> StatusMessage {
    StatusMessage::NewSong {
        tempo_bpm: 96.0,
        meter_num: 4,
        meter_den: 4,
        track: Some(piano()),
    }
}

#[test]
fn a_new_song() {
    let state = fresh().view();
    let song = &state.song;
    assert_eq!((song.undo_count, song.note_count), (0, 0));
    assert_eq!(
        state.timeline,
        TimelineView {
            tempo_bpm: 96.0,
            meter_num: 4,
            meter_den: 4,
            head_ramp: false,
            tempo_points: vec![],
            meter_changes: vec![],
            markers: vec![],
            selected: None,
            renaming: None,
            open: false,
        }
    );
    assert_eq!(song.play_target, Some(1));
    assert_eq!(
        song.tracks,
        [TrackView {
            kind: meloet_protocol::TrackKindView::Keys,
            id: 1,
            number: 1,
            name: "ピアノ".into(),
            color: 1,
            sound: SoundView::Builtin {
                instrument: InstrumentKind::GrandPiano
            },
            // This model was told of no sound pack.
            stand_in: Some(StandIn::PackMissing),
            drum_keys: None,
            gain_db: Some(0.0),
            pan: 0,
            sends: [Some(-15.0), None],
            send_defaults: [Some(-15.0), None],
            slots: [const { None }; 4],
            muted: false,
            soloed: false,
            lanes: vec![],
            clips: vec![],
            arp: Default::default(),
        }]
    );
    assert!(state.library.pack_missing);
    assert_eq!(
        state.library,
        LibraryView {
            pack_missing: true,
            ..LibraryView::default()
        }
    );
    assert_eq!(state.workspace.file_drop, None);
    let first_slot = |id, kind| [Some(InsertView::builtin(id, kind)), None, None, None];
    assert_eq!(
        song.returns,
        [(1, EffectKind::Reverb), (2, EffectKind::Delay)].map(|(id, kind)| ReturnView {
            gain_db: Some(0.0),
            pan: 0,
            muted: false,
            slots: first_slot(id, kind),
        })
    );
    assert_eq!(
        song.master,
        MasterView {
            gain_db: Some(0.0),
            pan: 0,
            slots: first_slot(3, EffectKind::Limiter),
            lanes: Vec::new(),
        }
    );
    assert_eq!(state.params, ParamsView::default());
    assert_eq!(state.workspace.scale, 100);
    assert_eq!(state.workspace.hint, None);
    assert_eq!(state.audio.requested_frames, 256);
    assert_eq!(state.audio.system, "PipeWire");
    assert!(!state.audio.running);
}

#[test]
fn keyboards_active_is_the_last_played() {
    let mut m = fresh();
    m.handle(Input::KeyboardAdded(keyboard(1)));
    m.handle(Input::KeyboardAdded(keyboard(2)));
    assert_eq!(m.view().keyboards.active.as_deref(), Some("kb2"));

    assert!(
        m.handle(Input::KeyboardPlayed { id: "kb1".into() })
            .is_empty()
    );
    let view = m.view();
    assert_eq!(view.keyboards.active.as_deref(), Some("kb1"));
    assert_eq!(view.keyboards.list.len(), 2);
    assert_eq!(
        view.audio.midi_input.as_deref(),
        Some("Keystation 1:MIDI 1 21:0")
    );

    // A keyboard plugged in later does not take the name from the one being played.
    m.handle(Input::KeyboardAdded(keyboard(3)));
    assert_eq!(m.view().keyboards.active.as_deref(), Some("kb1"));
    m.handle(Input::KeyboardPlayed { id: "kb2".into() });
    assert_eq!(m.view().keyboards.active.as_deref(), Some("kb2"));

    // The played one is pulled: back to the one plugged in last.
    m.handle(Input::KeyboardRemoved { id: "kb2".into() });
    assert_eq!(m.view().keyboards.active.as_deref(), Some("kb3"));
    m.handle(Input::KeyboardAdded(keyboard(2)));
    assert_eq!(m.view().keyboards.active.as_deref(), Some("kb2"));

    // A keyboard that is not there changes nothing.
    m.handle(Input::KeyboardPlayed { id: "no".into() });
    assert_eq!(m.view().keyboards.active.as_deref(), Some("kb2"));
}

#[test]
fn keyboards_active_is_the_last_plugged_before_any_key_and_lost_is_the_last_pulled() {
    let mut m = fresh();
    m.handle(Input::KeyboardAdded(keyboard(1)));
    m.handle(Input::KeyboardAdded(keyboard(2)));
    let view = m.view();
    assert_eq!(view.keyboards.active.as_deref(), Some("kb2"));
    assert_eq!(view.keyboards.list.len(), 2);
    assert_eq!(view.keyboards.list[0].address, "21:0");
    assert_eq!(
        view.audio.midi_input.as_deref(),
        Some("Keystation 2:MIDI 1 22:0")
    );

    let effects = m.handle(Input::KeyboardRemoved { id: "kb2".into() });
    assert_eq!(effects, [Effect::AllNotesOff]);
    let view = m.view();
    assert_eq!(view.keyboards.active.as_deref(), Some("kb1"));
    assert_eq!(view.keyboards.lost, None);

    m.handle(Input::KeyboardRemoved { id: "kb1".into() });
    let view = m.view();
    assert_eq!(view.keyboards.active, None);
    assert_eq!(view.keyboards.lost.as_deref(), Some("Keystation 1"));
    assert_eq!(view.audio.midi_input, None);

    m.handle(Input::KeyboardAdded(keyboard(2)));
    assert_eq!(m.view().keyboards.lost, None);
    assert!(
        m.handle(Input::KeyboardRemoved { id: "no".into() })
            .is_empty()
    );
}

#[test]
fn outputs_are_all_listed_in_chooser_order_and_only_some_can_be_chosen() {
    let mut m = fresh();
    m.handle(usual());
    let view = m.view();
    let rows: Vec<_> = view
        .outputs
        .list
        .iter()
        .map(|e| (e.id.as_str(), e.unusable, e.device.as_deref()))
        .collect();
    assert_eq!(
        rows,
        [
            (HP, None, Some("card device")),
            (USB, None, Some("usb device")),
            (HDMI, Some(Unusable::StoppedByPc), Some("gpu device")),
            (LINE, Some(Unusable::SwitchedByPc), Some("card device")),
        ]
    );
    assert_eq!(view.outputs.list[0].friendly, Some(Friendly::Headphones));
    assert_eq!(view.outputs.list[0].node_name.as_deref(), Some(HP_NODE));

    assert_eq!(view.inputs.list.len(), 1);
    assert_eq!(view.inputs.list[0].unusable, None);
    assert_eq!(view.inputs.chosen, None);
}

#[test]
fn chooser_shows_only_with_a_keyboard_and_no_chosen_output() {
    let mut m = fresh();
    m.handle(usual());
    assert_eq!(m.view().workspace.chooser, None);

    m.handle(Input::KeyboardAdded(keyboard(1)));
    assert_eq!(
        m.view().workspace.chooser,
        Some(ChooserView {
            highlighted: Some(HP.into())
        })
    );

    m.handle(Input::KeyboardRemoved { id: "kb1".into() });
    assert_eq!(m.view().workspace.chooser, None);

    let mut m = remembering(HP);
    m.handle(usual());
    m.handle(Input::KeyboardAdded(keyboard(1)));
    assert_eq!(m.view().workspace.chooser, None);
}

#[test]
fn chooser_highlight_waits_for_the_outputs() {
    let mut m = fresh();
    m.handle(Input::KeyboardAdded(keyboard(1)));
    assert_eq!(
        m.view().workspace.chooser,
        Some(ChooserView { highlighted: None })
    );
    assert!(cmd(&mut m, Command::ChooserConfirm).is_empty());
    m.handle(usual());
    assert_eq!(
        m.view().workspace.chooser.unwrap().highlighted.as_deref(),
        Some(HP)
    );
}

#[test]
fn chooser_move_skips_what_cannot_be_chosen() {
    let highlighted = |m: &Model| m.view().workspace.chooser.unwrap().highlighted.unwrap();
    let mut m = fresh();
    m.handle(Input::KeyboardAdded(keyboard(1)));
    m.handle(usual());

    cmd(&mut m, Command::ChooserMove { delta: 1 });
    assert_eq!(highlighted(&m), USB);
    cmd(&mut m, Command::ChooserMove { delta: 1 });
    assert_eq!(highlighted(&m), USB);
    cmd(&mut m, Command::ChooserMove { delta: -1 });
    assert_eq!(highlighted(&m), HP);
    cmd(&mut m, Command::ChooserMove { delta: -1 });
    assert_eq!(highlighted(&m), HP);

    cmd(
        &mut m,
        Command::ChooserSelect {
            output: HDMI.into(),
        },
    );
    assert_eq!(highlighted(&m), HP);
    cmd(&mut m, Command::ChooserSelect { output: USB.into() });
    assert_eq!(highlighted(&m), USB);

    // The highlighted output stops being choosable: back to the top one.
    m.handle(picture(vec![headphones(Av::Yes, true)], None));
    assert_eq!(highlighted(&m), HP);
}

#[test]
fn chooser_confirm_chooses_saves_and_opens() {
    let mut m = fresh();
    m.handle(Input::KeyboardAdded(keyboard(1)));
    m.handle(usual());
    cmd(&mut m, Command::ChooserMove { delta: 1 });

    let effects = cmd(&mut m, Command::ChooserConfirm);
    assert_eq!(
        effects,
        [
            Effect::SaveSettings(Box::new(Settings {
                output: Some(USB.into()),
                output_name: Some("USB スピーカー".into()),
                scale: 100,
                ..Settings::default()
            })),
            open(USB_NODE),
        ]
    );
    let view = m.view();
    assert_eq!(view.outputs.chosen.as_deref(), Some(USB));
    assert_eq!(view.outputs.chosen_state, Some(ChosenState::Ready));
    assert_eq!(view.workspace.chooser, None);
    assert!(!view.audio.running);

    opened(&mut m, USB_NODE, 44_100);
    let audio = m.view().audio;
    assert!(audio.running);
    assert_eq!(audio.sample_rate, Some(44_100));
    assert_eq!(audio.output_device.as_deref(), Some(USB_NODE));
    assert_eq!(audio.actual_frames, None);
    assert_eq!(audio.latency_ms, None);

    assert!(cmd(&mut m, Command::ChooserConfirm).is_empty());
}

#[test]
fn test_tone_opens_the_output_without_choosing_it() {
    let mut m = fresh();
    m.handle(Input::KeyboardAdded(keyboard(1)));
    m.handle(usual());

    assert!(
        cmd(
            &mut m,
            Command::TestTone {
                output: HDMI.into()
            }
        )
        .is_empty()
    );
    assert!(cmd(&mut m, Command::TestTone { output: "?".into() }).is_empty());

    let effects = cmd(&mut m, Command::TestTone { output: USB.into() });
    assert_eq!(effects, [open(USB_NODE), Effect::TestTone]);
    assert_eq!(m.view().outputs.chosen, None);
    opened(&mut m, USB_NODE, 48_000);

    // Already open there: only the tone.
    let effects = cmd(&mut m, Command::TestTone { output: USB.into() });
    assert_eq!(effects, [Effect::TestTone]);

    // Deciding on another output moves the stream.
    let effects = cmd(&mut m, Command::ChooserConfirm);
    assert_eq!(effects.last(), Some(&open(HP_NODE)));
}

#[test]
fn a_tested_output_is_let_go_when_the_chooser_goes_away() {
    let mut m = fresh();
    m.handle(Input::KeyboardAdded(keyboard(1)));
    m.handle(usual());
    cmd(&mut m, Command::TestTone { output: USB.into() });
    opened(&mut m, USB_NODE, 48_000);

    let effects = m.handle(Input::KeyboardRemoved { id: "kb1".into() });
    assert_eq!(effects, [Effect::AllNotesOff, Effect::CloseOutput]);
    assert!(!m.view().audio.running);
}

#[test]
fn a_failed_open_is_not_tried_again_until_the_devices_change() {
    let mut m = remembering(HP);
    assert_eq!(m.handle(usual()), [open(HP_NODE)]);
    let effects = m.handle(Input::AudioOpenFailed {
        node_name: HP_NODE.into(),
    });
    assert!(effects.is_empty());
    assert!(
        m.handle(Input::KeyboardAdded(keyboard(1))).is_empty(),
        "no retry on unrelated news"
    );
    assert!(!m.view().audio.running);
    assert_eq!(m.handle(usual()), [open(HP_NODE)]);
}

#[test]
fn remembered_output_that_is_not_there_at_start_is_not_plugged() {
    // Not plugged in.
    let mut m = remembering(HP);
    assert_eq!(m.view().outputs.chosen_state, None, "no picture yet");
    let effects = m.handle(picture(vec![headphones(Av::No, true), usb()], None));
    assert!(effects.is_empty());
    assert_eq!(m.view().outputs.chosen_state, Some(ChosenState::NotPlugged));
    assert_eq!(
        status(&m),
        Some(StatusMessage::OutputNotPlugged {
            output: "アナログヘッドフォン".into()
        })
    );

    // The PC has another Route on: still not meloet's to open.
    let effects = m.handle(picture(vec![headphones(Av::Yes, false), usb()], None));
    assert!(effects.is_empty());
    assert_eq!(m.view().outputs.chosen_state, Some(ChosenState::NotPlugged));

    let effects = m.handle(picture(vec![headphones(Av::Yes, true), usb()], None));
    assert_eq!(effects, [open(HP_NODE)]);
    assert_eq!(m.view().outputs.chosen_state, Some(ChosenState::Ready));
    assert_eq!(status(&m), Some(StatusMessage::SearchingKeyboard));

    // Not in the list at all: the remembered name is used.
    let mut m = remembering(HP);
    assert!(m.handle(picture(vec![usb()], None)).is_empty());
    assert_eq!(m.view().outputs.chosen_state, Some(ChosenState::NotPlugged));
    assert_eq!(
        status(&m),
        Some(StatusMessage::OutputNotPlugged {
            output: "アナログヘッドフォン".into()
        })
    );
    assert_eq!(
        m.handle(picture(vec![usb(), headphones(Av::Unknown, true)], None)),
        [open(HP_NODE)]
    );
}

#[test]
fn output_lost_while_running_is_missing_and_closed_then_comes_back() {
    for gone in [
        vec![headphones(Av::No, true), usb()],
        vec![headphones(Av::Yes, false), usb()],
        vec![usb()],
    ] {
        let mut m = remembering(HP);
        m.handle(Input::KeyboardAdded(keyboard(1)));
        assert_eq!(m.handle(usual()), [open(HP_NODE)]);
        opened(&mut m, HP_NODE, 48_000);
        m.handle(Input::AudioFrames(Some(256)));

        let effects = m.handle(picture(gone, None));
        assert_eq!(effects, [Effect::CloseOutput]);
        let view = m.view();
        assert_eq!(view.outputs.chosen_state, Some(ChosenState::Missing));
        assert_eq!(
            view.status.message,
            Some(StatusMessage::OutputMissing {
                output: "アナログヘッドフォン".into()
            })
        );
        assert!(!view.audio.running);
        assert_eq!(view.audio.sample_rate, None);
        assert_eq!(view.audio.actual_frames, None);
        assert_eq!(view.audio.latency_ms, None);
        assert_eq!(view.audio.latency_ms_at_requested, None);
        assert_eq!(view.audio.output_device, None);

        assert_eq!(m.handle(usual()), [open(HP_NODE)]);
        assert_eq!(m.view().outputs.chosen_state, Some(ChosenState::Ready));
        assert_eq!(status(&m), Some(new_song()));
    }
}

#[test]
fn settings_dialog() {
    let mut m = playing();
    let dialog = |m: &Model| m.view().workspace.settings;

    cmd(&mut m, Command::OpenSettings { advanced: false });
    assert_eq!(
        dialog(&m),
        Some(SettingsDialogView {
            draft_output: Some(HP.into()),
            advanced_open: false
        })
    );
    assert_eq!(status(&m), Some(StatusMessage::SettingsOpen));
    cmd(&mut m, Command::SettingsToggleAdvanced);
    assert!(dialog(&m).unwrap().advanced_open);
    cmd(&mut m, Command::SettingsToggleAdvanced);
    assert!(!dialog(&m).unwrap().advanced_open);
    cmd(&mut m, Command::OpenSettings { advanced: true });
    assert!(dialog(&m).unwrap().advanced_open);

    // Only what can be chosen.
    cmd(
        &mut m,
        Command::SettingsSelectOutput {
            output: LINE.into(),
        },
    );
    assert_eq!(dialog(&m).unwrap().draft_output.as_deref(), Some(HP));
    cmd(&mut m, Command::SettingsSelectOutput { output: USB.into() });
    assert_eq!(dialog(&m).unwrap().draft_output.as_deref(), Some(USB));
    assert_eq!(m.view().outputs.chosen.as_deref(), Some(HP));

    // Cancel: nothing changed.
    assert!(cmd(&mut m, Command::SettingsCancel).is_empty());
    assert_eq!(dialog(&m), None);
    assert_eq!(m.view().outputs.chosen.as_deref(), Some(HP));
    assert_eq!(status(&m), idle());

    // Opens with the advanced part.
    cmd(&mut m, Command::OpenSettings { advanced: true });
    assert_eq!(
        dialog(&m),
        Some(SettingsDialogView {
            draft_output: Some(HP.into()),
            advanced_open: true
        })
    );

    // Apply: chosen, saved, reopened, closed.
    cmd(&mut m, Command::SettingsSelectOutput { output: USB.into() });
    let effects = cmd(&mut m, Command::SettingsApply);
    assert_eq!(
        effects,
        [
            Effect::SaveSettings(Box::new(Settings {
                output: Some(USB.into()),
                output_name: Some("USB スピーカー".into()),
                scale: 100,
                ..Settings::default()
            })),
            open(USB_NODE),
        ]
    );
    assert_eq!(dialog(&m), None);
    assert_eq!(m.view().outputs.chosen.as_deref(), Some(USB));

    // Apply without a change does nothing but close.
    opened(&mut m, USB_NODE, 48_000);
    cmd(&mut m, Command::OpenSettings { advanced: false });
    assert!(cmd(&mut m, Command::SettingsApply).is_empty());
    assert_eq!(dialog(&m), None);
    assert!(m.view().audio.running);

    // Commands for the dialog do nothing while it is closed.
    cmd(&mut m, Command::SettingsSelectOutput { output: HP.into() });
    cmd(&mut m, Command::SettingsToggleAdvanced);
    assert!(cmd(&mut m, Command::SettingsApply).is_empty());
    assert_eq!(dialog(&m), None);
    assert_eq!(m.view().outputs.chosen.as_deref(), Some(USB));
}

#[test]
fn back_closes_the_dialog_first_and_leaves_the_chooser() {
    let mut m = playing();
    cmd(
        &mut m,
        Command::ToggleBand {
            band: Band::PianoRoll,
        },
    );
    cmd(&mut m, Command::OpenSettings { advanced: false });
    cmd(&mut m, Command::SettingsSelectOutput { output: USB.into() });

    assert!(cmd(&mut m, Command::Back).is_empty());
    let view = m.view();
    assert_eq!(view.workspace.settings, None);
    assert_eq!(view.outputs.chosen.as_deref(), Some(HP));
    assert_eq!(view.workspace.layout.lower.len(), 1);

    cmd(&mut m, Command::Back);
    assert!(m.view().workspace.layout.lower.is_empty());

    let mut m = fresh();
    m.handle(Input::KeyboardAdded(keyboard(1)));
    m.handle(usual());
    cmd(&mut m, Command::Back);
    assert!(m.view().workspace.chooser.is_some());
}

#[test]
fn a_tone_in_the_dialog_goes_back_to_the_chosen_output_on_cancel() {
    let mut m = playing();
    cmd(&mut m, Command::OpenSettings { advanced: false });
    cmd(&mut m, Command::SettingsSelectOutput { output: USB.into() });
    let effects = cmd(&mut m, Command::TestTone { output: USB.into() });
    assert_eq!(effects, [open(USB_NODE), Effect::TestTone]);
    opened(&mut m, USB_NODE, 48_000);
    assert_eq!(m.view().outputs.chosen.as_deref(), Some(HP));

    assert_eq!(cmd(&mut m, Command::SettingsCancel), [open(HP_NODE)]);

    // Outside the chooser and the dialog a tone never moves the stream.
    opened(&mut m, HP_NODE, 48_000);
    assert!(cmd(&mut m, Command::TestTone { output: USB.into() }).is_empty());
    assert_eq!(
        cmd(&mut m, Command::TestTone { output: HP.into() }),
        [Effect::TestTone]
    );
}

#[test]
fn buffer_fixed_by_the_pc() {
    let [hdmi, line] = others();
    let outputs = || vec![headphones(Av::Yes, true), hdmi.clone(), line.clone(), usb()];
    let mut m = playing();
    m.handle(picture(outputs(), Some(1024)));
    m.handle(Input::AudioFrames(Some(1024)));
    let view = m.view();
    let Some(StatusMessage::BufferFixed {
        actual,
        requested,
        latency_ms,
        latency_ms_at_requested,
    }) = view.status.message
    else {
        panic!("{:?}", view.status);
    };
    assert_eq!((actual, requested), (1024, 256));
    assert!((latency_ms - 53.33).abs() < 0.01, "{latency_ms}");
    assert!(
        (latency_ms_at_requested - 13.33).abs() < 0.01,
        "{latency_ms_at_requested}"
    );
    assert_eq!(view.audio.forced_quantum, Some(1024));
    assert_eq!(view.audio.actual_frames, Some(1024));
    assert!((view.audio.latency_ms.unwrap() - 53.33).abs() < 0.01);
    assert!((view.audio.latency_ms_at_requested.unwrap() - 13.33).abs() < 0.01);

    // Not forced, and running at another size: said in the audio section only.
    let mut m = playing();
    m.handle(Input::AudioFrames(Some(128)));
    let view = m.view();
    assert_eq!(view.status.message, idle());
    assert_eq!(view.audio.actual_frames, Some(128));
    assert_eq!(view.audio.forced_quantum, None);
    assert!((view.audio.latency_ms.unwrap() - 6.67).abs() < 0.01);

    // Forced to a size other than the one running.
    m.handle(picture(outputs(), Some(1024)));
    assert_eq!(status(&m), idle());

    // Forced to the size that was asked for.
    m.handle(picture(outputs(), Some(256)));
    m.handle(Input::AudioFrames(Some(256)));
    assert_eq!(status(&m), idle());
}

#[test]
fn buffer_fixed_is_said_when_a_song_opens_and_when_the_size_changes() {
    let [hdmi, line] = others();
    let outputs = || vec![headphones(Av::Yes, true), hdmi.clone(), line.clone(), usb()];
    let fixed_at = |m: &Model| match status(m) {
        Some(StatusMessage::BufferFixed { actual, .. }) => Some(actual),
        _ => None,
    };
    let right_end = |m: &Model| {
        let audio = m.view().audio;
        (audio.actual_frames, audio.requested_frames)
    };

    // At start the size is not known until the first block.
    let mut m = playing();
    m.handle(picture(outputs(), Some(1024)));
    assert_eq!(status(&m), idle());
    m.handle(Input::AudioFrames(Some(1024)));
    assert_eq!(fixed_at(&m), Some(1024));
    assert_eq!(right_end(&m), (Some(1024), 256));

    // A resize leaves it; the next thing the user does takes it away. The right end stays.
    viewport(&mut m, 1300.0, 820.0);
    assert_eq!(fixed_at(&m), Some(1024));
    cmd(&mut m, Command::DismissHint);
    assert_eq!(status(&m), idle());
    assert_eq!(right_end(&m), (Some(1024), 256));

    // The same size again is not said.
    m.handle(Input::AudioFrames(Some(1024)));
    m.handle(picture(outputs(), Some(1024)));
    assert_eq!(status(&m), idle());

    // Nor on another output that runs at the same size.
    cmd(&mut m, Command::OpenSettings { advanced: false });
    cmd(&mut m, Command::SettingsSelectOutput { output: USB.into() });
    cmd(&mut m, Command::SettingsApply);
    opened(&mut m, USB_NODE, 48_000);
    m.handle(Input::AudioFrames(Some(1024)));
    assert_eq!(status(&m), idle());

    // A new size is.
    m.handle(picture(outputs(), Some(2048)));
    assert_eq!(status(&m), idle(), "not running at it yet");
    m.handle(Input::AudioFrames(Some(2048)));
    assert_eq!(fixed_at(&m), Some(2048));

    // Opening a song says it again, right away while the size is known.
    cmd(&mut m, Command::DismissHint);
    m.handle(Input::SongOpened);
    assert_eq!(fixed_at(&m), Some(2048));

    // Not fixed: nothing is said, not even when a song opens.
    let mut m = playing();
    m.handle(Input::AudioFrames(Some(128)));
    m.handle(Input::SongOpened);
    assert_eq!(status(&m), Some(new_song()), "the song, not the buffer");
    assert_eq!(right_end(&m), (Some(128), 256));
    // Fixed after not being fixed is a change.
    m.handle(picture(outputs(), Some(1024)));
    m.handle(Input::AudioFrames(Some(1024)));
    assert_eq!(fixed_at(&m), Some(1024));

    // A test tone on another output in the chooser does not use up the song's turn.
    let mut m = fresh();
    m.handle(Input::KeyboardAdded(keyboard(1)));
    m.handle(picture(outputs(), Some(1024)));
    cmd(&mut m, Command::TestTone { output: USB.into() });
    opened(&mut m, USB_NODE, 48_000);
    m.handle(Input::AudioFrames(Some(1024)));
    assert!(matches!(
        status(&m),
        Some(StatusMessage::KeyboardFound { .. })
    ));
    cmd(&mut m, Command::ChooserSelect { output: HP.into() });
    cmd(&mut m, Command::ChooserConfirm);
    opened(&mut m, HP_NODE, 48_000);
    m.handle(Input::AudioFrames(Some(1024)));
    assert_eq!(fixed_at(&m), Some(1024));
}

#[test]
fn measured_latency_is_the_mean_of_the_last_16() {
    let mut m = playing();
    m.handle(Input::AudioFrames(Some(256)));
    assert!((m.view().audio.latency_ms.unwrap() - 13.33).abs() < 0.01);

    m.handle(Input::LatencySample { ms: 12.0 });
    m.handle(Input::LatencySample { ms: 14.0 });
    assert_eq!(m.view().audio.latency_ms, Some(13.0));
    for _ in 0..16 {
        m.handle(Input::LatencySample { ms: 15.04 });
    }
    assert_eq!(m.view().audio.latency_ms, Some(15.0));

    // A new stream starts its own average.
    cmd(&mut m, Command::OpenSettings { advanced: false });
    cmd(&mut m, Command::SettingsSelectOutput { output: USB.into() });
    cmd(&mut m, Command::SettingsApply);
    m.handle(Input::LatencySample { ms: 99.0 });
    assert_eq!(m.view().audio.latency_ms, None, "not open yet");
    opened(&mut m, USB_NODE, 48_000);
    assert_eq!(m.view().audio.latency_ms, None);
}

#[test]
fn status_table_in_order() {
    use StatusMessage::*;
    let mut m = fresh();
    m.handle(usual());
    assert_eq!(status(&m), Some(SearchingKeyboard));

    m.handle(Input::KeyboardAdded(keyboard(1)));
    assert_eq!(
        status(&m),
        Some(KeyboardFound {
            keyboard: "Keystation 1".into()
        })
    );

    m.handle(Input::KeyboardRemoved { id: "kb1".into() });
    assert_eq!(
        status(&m),
        Some(KeyboardLost {
            keyboard: "Keystation 1".into()
        })
    );

    // 1 wins over everything.
    cmd(&mut m, Command::OpenSettings { advanced: false });
    assert_eq!(status(&m), Some(SettingsOpen));
    cmd(&mut m, Command::SettingsCancel);

    m.handle(Input::KeyboardAdded(keyboard(1)));
    cmd(&mut m, Command::ChooserConfirm);
    opened(&mut m, HP_NODE, 48_000);
    assert_eq!(
        status(&m),
        Some(OutputChosen {
            output: "アナログヘッドフォン".into(),
            track: Some(piano())
        })
    );

    // 7, then 4 and 5 over 7, 2 over 4, 1 over 2.
    let [hdmi, line] = others();
    m.handle(picture(
        vec![headphones(Av::Yes, true), hdmi, line, usb()],
        Some(1024),
    ));
    m.handle(Input::AudioFrames(Some(1024)));
    assert!(matches!(status(&m), Some(BufferFixed { .. })));
    m.handle(Input::KeyboardRemoved { id: "kb1".into() });
    assert!(matches!(status(&m), Some(KeyboardLost { .. })));
    m.handle(picture(vec![headphones(Av::No, true)], Some(1024)));
    assert_eq!(
        status(&m),
        Some(OutputMissing {
            output: "アナログヘッドフォン".into()
        })
    );
    cmd(&mut m, Command::OpenSettings { advanced: true });
    assert_eq!(status(&m), Some(SettingsOpen));
    cmd(&mut m, Command::Back);
    assert!(matches!(status(&m), Some(OutputMissing { .. })));

    // 3 over 5 and 6.
    let mut m = remembering(HP);
    m.handle(picture(vec![headphones(Av::No, true)], None));
    assert!(matches!(status(&m), Some(OutputNotPlugged { .. })));
    m.handle(Input::KeyboardAdded(keyboard(1)));
    assert!(matches!(status(&m), Some(OutputNotPlugged { .. })));

    // 5 with a chosen output that is ready.
    let mut m = remembering(HP);
    m.handle(usual());
    assert_eq!(status(&m), Some(SearchingKeyboard));
}

#[test]
fn sound_came_out_once_after_the_chooser_decided() {
    let hint = |m: &Model| m.view().workspace.hint;
    let mut m = fresh();
    m.handle(Input::KeyboardAdded(keyboard(1)));
    m.handle(usual());
    cmd(&mut m, Command::TestTone { output: HP.into() });
    opened(&mut m, HP_NODE, 48_000);

    // Sound before the decision (the test tone) does not count.
    assert!(!m.wants_sound());
    m.handle(Input::SoundHeard);
    assert_eq!(hint(&m), None);
    cmd(&mut m, Command::DismissHint);

    cmd(&mut m, Command::ChooserConfirm);
    assert!(m.wants_sound());
    assert_eq!(hint(&m), Some(Hint::EmptyArrange));
    m.handle(Input::SoundHeard);
    assert_eq!(hint(&m), Some(Hint::SoundCameOut));
    assert!(!m.wants_sound());

    // The empty arrange hint comes back behind it, until it is put away too.
    cmd(&mut m, Command::DismissHint);
    assert_eq!(hint(&m), Some(Hint::EmptyArrange));
    m.handle(Input::SoundHeard);
    assert_eq!(hint(&m), Some(Hint::EmptyArrange));
    assert!(!m.wants_sound());
    cmd(&mut m, Command::DismissHint);
    assert_eq!(hint(&m), None);
}

#[test]
fn an_output_remembered_from_before_gives_no_hint() {
    let mut m = remembering(HP);
    m.handle(Input::KeyboardAdded(keyboard(1)));
    m.handle(usual());
    opened(&mut m, HP_NODE, 48_000);
    m.handle(Input::SoundHeard);
    assert_eq!(m.view().workspace.hint, Some(Hint::EmptyArrange));

    // Nor does a change in the settings dialog: only the chooser arms it.
    cmd(&mut m, Command::OpenSettings { advanced: false });
    cmd(&mut m, Command::SettingsSelectOutput { output: USB.into() });
    cmd(&mut m, Command::SettingsApply);
    assert_eq!(m.view().outputs.chosen.as_deref(), Some(USB));
    assert!(!m.wants_sound());
}

#[test]
fn bands_and_the_scale() {
    let layout = |m: &Model| m.view().workspace.layout;
    let mut m = fresh();
    let l = layout(&m);
    assert_eq!(l.meter, MeterSize::Full);
    assert_eq!(l.arrange_height, 800.0 - 24.0 - 48.0 - 52.0 - 24.0 - 24.0);
    assert!(!l.arrange_overview);

    cmd(
        &mut m,
        Command::ToggleBand {
            band: Band::PianoRoll,
        },
    );
    let l = layout(&m);
    assert_eq!(l.meter, MeterSize::Slim);
    assert!(l.arrange_overview);
    assert_eq!(l.arrange_height, 40.0);
    assert_eq!(
        l.lower,
        [LowerBandView {
            band: Band::PianoRoll,
            height: 800.0 - 24.0 - 48.0 - 24.0 - 40.0 - 24.0 - 24.0
        }]
    );
    assert!(l.show_back_hint);

    cmd(&mut m, Command::ToggleBand { band: Band::Shelf });
    cmd(&mut m, Command::ToggleBand { band: Band::Params });
    let l = layout(&m);
    assert_eq!(l.right_open, [Band::Shelf, Band::Params]);
    assert_eq!(l.right_front, Some(Band::Params));

    cmd(&mut m, Command::GoArrange);
    let l = layout(&m);
    assert!(l.lower.is_empty());
    assert_eq!(l.right_front, Some(Band::Params));

    cmd(&mut m, Command::ToggleBand { band: Band::Mixer });
    cmd(&mut m, Command::Back);
    assert_eq!(layout(&m).right_open, [Band::Shelf]);
    cmd(&mut m, Command::Back);
    assert!(layout(&m).right_open.is_empty());
    assert_eq!(layout(&m).lower[0].band, Band::Mixer);
    cmd(&mut m, Command::Back);
    assert!(layout(&m).lower.is_empty());
    cmd(&mut m, Command::Back);
    assert_eq!(layout(&m), layout(&fresh()));
}

#[test]
fn viewport_is_css_px_and_the_rules_work_at_100_percent() {
    let mut m = fresh();
    // 1200 CSS px at 125 % is 960 at 100 %.
    let effects = cmd(&mut m, Command::SetScale { percent: 125 });
    assert_eq!(
        effects,
        [Effect::SaveSettings(Box::new(Settings {
            output: None,
            output_name: None,
            scale: 125,
            ..Settings::default()
        }))]
    );
    cmd(
        &mut m,
        Command::SetViewport {
            width: 1600.0,
            height: 1200.0,
        },
    );
    cmd(&mut m, Command::ToggleBand { band: Band::Mixer });
    let ws = m.view().workspace;
    assert_eq!(
        (ws.scale, ws.viewport_width, ws.viewport_height),
        (125, 1600.0, 1200.0)
    );
    assert_eq!(ws.layout.meter, MeterSize::Slim);
    assert_eq!(ws.layout.lower[0].height, 660.0);
    assert_eq!(
        ws.layout.arrange_height,
        960.0 - 24.0 - 48.0 - 24.0 - 660.0 - 24.0 - 24.0
    );

    cmd(
        &mut m,
        Command::SetViewport {
            width: 1600.0,
            height: 900.0,
        },
    );
    let l = m.view().workspace.layout;
    assert_eq!(l.meter, MeterSize::Slim);
    assert_eq!(l.arrange_height, 40.0);
    assert_eq!(
        l.lower[0].height,
        720.0 - 24.0 - 48.0 - 24.0 - 40.0 - 24.0 - 24.0
    );

    // Other scales and broken sizes are ignored.
    for percent in [0, 99, 110, 150, 200] {
        assert!(cmd(&mut m, Command::SetScale { percent }).is_empty());
    }
    assert!(cmd(&mut m, Command::SetScale { percent: 125 }).is_empty());
    for (width, height) in [(0.0, 800.0), (1280.0, -1.0), (f32::NAN, 800.0)] {
        cmd(&mut m, Command::SetViewport { width, height });
    }
    let ws = m.view().workspace;
    assert_eq!(
        (ws.scale, ws.viewport_width, ws.viewport_height),
        (125, 1600.0, 900.0)
    );

    assert_eq!(
        cmd(&mut m, Command::SetScale { percent: 100 }).len(),
        1,
        "saved"
    );
    assert_eq!(m.view().workspace.scale, 100);
}

#[test]
fn a_theme_is_a_setting_and_not_a_step() {
    let mut m = fresh();
    assert_eq!(m.view().workspace.theme, Theme::Dark);
    assert!(
        cmd(&mut m, Command::SetTheme { theme: Theme::Dark }).is_empty(),
        "the same theme"
    );
    let before = m.view();
    let effects = cmd(
        &mut m,
        Command::SetTheme {
            theme: Theme::MintDark,
        },
    );
    assert_eq!(
        effects,
        [Effect::SaveSettings(Box::new(Settings {
            theme: Theme::MintDark,
            ..Settings::default()
        }))]
    );
    let after = m.view();
    assert_eq!(after.workspace.theme, Theme::MintDark);
    assert_eq!(
        AppState::changes_from(&before, &after),
        [meloet_protocol::Change::Workspace(after.workspace.clone())]
    );
    assert_eq!(after.song.undo_count, 0);

    let m = Model::new(Settings {
        theme: Theme::Light,
        ..Settings::default()
    });
    assert_eq!(m.view().workspace.theme, Theme::Light);
}

/// E1 「表示 ▸ 鳴っている音を光らせる」: a meloet setting like the theme, on at first, taken while
/// recording too, never a step.
#[test]
fn the_note_glow_is_a_setting_and_not_a_step() {
    let mut m = fresh();
    assert!(m.view().workspace.note_glow);
    assert!(
        cmd(&mut m, Command::SetNoteGlow { on: true }).is_empty(),
        "already on"
    );
    let before = m.view();
    let effects = cmd(&mut m, Command::SetNoteGlow { on: false });
    assert_eq!(
        effects,
        [Effect::SaveSettings(Box::new(Settings {
            note_glow: false,
            ..Settings::default()
        }))]
    );
    let after = m.view();
    assert!(!after.workspace.note_glow);
    assert_eq!(
        AppState::changes_from(&before, &after),
        [meloet_protocol::Change::Workspace(after.workspace.clone())]
    );
    assert_eq!(after.song.undo_count, 0);

    let mut m = playing();
    cmd(&mut m, Command::Record);
    assert!(m.play.busy());
    let effects = cmd(&mut m, Command::SetNoteGlow { on: false });
    assert!(matches!(effects.as_slice(), [Effect::SaveSettings(s)] if !s.note_glow));
    assert!(!m.view().workspace.note_glow);
}

#[test]
fn arrange_lower_than_one_track_row_is_drawn_as_the_overview() {
    let mut m = fresh();
    // The piano roll grows up to 760 before the arrange view does (with the slim meter, 144 more).
    cmd(
        &mut m,
        Command::ToggleBand {
            band: Band::PianoRoll,
        },
    );
    let l = m.view().workspace.layout;
    assert_eq!(l.arrange_height, 40.0);
    assert!(l.arrange_overview);

    for (height, arrange, overview) in [(1039.0, 135.0, true), (1040.0, 136.0, false)] {
        cmd(
            &mut m,
            Command::SetViewport {
                width: 1280.0,
                height,
            },
        );
        let l = m.view().workspace.layout;
        assert_eq!(l.arrange_height, arrange);
        assert_eq!(l.arrange_overview, overview);
    }
}

#[test]
fn the_scale_comes_from_the_settings() {
    let m = Model::new(Settings {
        output: None,
        output_name: None,
        scale: 125,
        ..Settings::default()
    });
    let ws = m.view().workspace;
    assert_eq!(ws.scale, 125);
    assert_eq!(
        ws.layout.arrange_height,
        800.0 / 1.25 - 24.0 - 48.0 - 52.0 - 24.0 - 24.0
    );
}

fn viewport(m: &mut Model, width: f32, height: f32) -> Vec<Effect> {
    cmd(m, Command::SetViewport { width, height })
}

fn toggle(m: &mut Model, band: Band) {
    cmd(m, Command::ToggleBand { band });
}

fn screen(m: &mut Model, width: f32, height: f32) -> Vec<Effect> {
    m.handle(Input::ScreenSpace { width, height })
}

fn open_bands(m: &Model) -> Vec<Band> {
    let lower = m.view().workspace.layout.lower;
    lower.iter().map(|b| b.band).collect()
}

fn saved(scale: u16) -> Effect {
    Effect::SaveSettings(Box::new(Settings {
        output: None,
        output_name: None,
        scale,
        ..Settings::default()
    }))
}

#[test]
fn a_lower_window_folds_the_older_band_and_says_so() {
    let mut m = playing();
    viewport(&mut m, 1280.0, 1080.0);
    toggle(&mut m, Band::Mixer);
    toggle(&mut m, Band::PianoRoll);
    assert_eq!(status(&m), idle());

    viewport(&mut m, 1280.0, 800.0);
    assert_eq!(open_bands(&m), [Band::PianoRoll]);
    let folded = Some(StatusMessage::BandFoldedWindowLow { band: Band::Mixer });
    assert_eq!(status(&m), folded);

    // More resizing leaves the notice; a taller window does not open the band again.
    viewport(&mut m, 1280.0, 1400.0);
    assert_eq!(open_bands(&m), [Band::PianoRoll]);
    assert_eq!(status(&m), folded);

    // The next thing the user does takes it away.
    toggle(&mut m, Band::Shelf);
    assert_eq!(status(&m), idle());
}

#[test]
fn opening_a_band_that_does_not_fit_beside_the_other_says_so() {
    let mut m = playing();
    toggle(&mut m, Band::Mixer);
    toggle(&mut m, Band::PianoRoll);
    assert_eq!(open_bands(&m), [Band::PianoRoll]);
    assert_eq!(
        status(&m),
        Some(StatusMessage::BandFoldedToOpen {
            opened: Band::PianoRoll,
            folded: Band::Mixer,
        })
    );
    cmd(&mut m, Command::GoArrange);
    assert_eq!(status(&m), idle());
}

#[test]
fn a_third_band_folds_the_oldest_and_says_so() {
    let mut m = playing();
    viewport(&mut m, 1280.0, 1080.0);
    toggle(&mut m, Band::Mixer);
    toggle(&mut m, Band::PianoRoll);
    toggle(&mut m, Band::History);
    assert_eq!(open_bands(&m), [Band::PianoRoll, Band::History]);
    assert_eq!(
        status(&m),
        Some(StatusMessage::BandFoldedThird {
            opened: Band::History,
            folded: Band::Mixer,
        })
    );
    cmd(&mut m, Command::SelectClip { clip: None });
    assert_eq!(status(&m), idle());

    // The third band pushes out the oldest, and the second one still does not fit beside it.
    let mut m = playing();
    viewport(&mut m, 1280.0, 940.0);
    toggle(&mut m, Band::History);
    toggle(&mut m, Band::Mixer);
    toggle(&mut m, Band::PianoRoll);
    assert_eq!(open_bands(&m), [Band::PianoRoll]);
    assert_eq!(
        status(&m),
        Some(StatusMessage::BandFoldedToOpen {
            opened: Band::PianoRoll,
            folded: Band::Mixer,
        })
    );
}

#[test]
fn a_notice_waits_behind_the_device_lines_and_goes_before_the_rest() {
    use StatusMessage::*;
    // No keyboard yet: the notice goes before "searching".
    let mut m = fresh();
    toggle(&mut m, Band::Mixer);
    toggle(&mut m, Band::PianoRoll);
    let notice = Some(BandFoldedToOpen {
        opened: Band::PianoRoll,
        folded: Band::Mixer,
    });
    assert_eq!(status(&m), notice);
    // Things the user did not do leave it.
    m.handle(usual());
    assert_eq!(status(&m), notice);

    // A keyboard that was pulled out goes first, and the notice is still there after it.
    m.handle(Input::KeyboardAdded(keyboard(1)));
    m.handle(Input::KeyboardRemoved { id: keyboard(1).id });
    assert!(matches!(status(&m), Some(KeyboardLost { .. })));
    m.handle(Input::KeyboardAdded(keyboard(1)));
    assert_eq!(status(&m), notice);

    // The settings dialog goes first; opening it is something the user did.
    let mut m = playing();
    viewport(&mut m, 1280.0, 1080.0);
    toggle(&mut m, Band::Mixer);
    toggle(&mut m, Band::PianoRoll);
    cmd(&mut m, Command::OpenSettings { advanced: false });
    viewport(&mut m, 1280.0, 800.0);
    assert_eq!(status(&m), Some(SettingsOpen));
    cmd(&mut m, Command::SettingsCancel);
    assert_eq!(status(&m), idle());
}

#[test]
fn unfit_scales_follow_the_screen() {
    let mut m = fresh();
    assert!(m.view().workspace.unfit_scales.is_empty());
    for ((width, height), unfit) in [
        ((1920.0, 1080.0), vec![]),
        ((1280.0, 800.0), vec![]),
        ((1279.0, 800.0), vec![125]),
        ((1280.0, 799.0), vec![125]),
        ((1200.0, 700.0), vec![125]),
        // 100 is never listed.
        ((800.0, 400.0), vec![125]),
    ] {
        assert!(screen(&mut m, width, height).is_empty());
        assert_eq!(m.view().workspace.unfit_scales, unfit, "{width}x{height}");
    }
    // Broken sizes are ignored.
    screen(&mut m, 1920.0, 1080.0);
    for (width, height) in [(0.0, 800.0), (1280.0, -1.0), (f32::NAN, 800.0)] {
        screen(&mut m, width, height);
    }
    assert!(m.view().workspace.unfit_scales.is_empty());
}

#[test]
fn a_scale_that_does_not_fit_is_not_taken() {
    let mut m = playing();
    screen(&mut m, 1200.0, 700.0);
    viewport(&mut m, 1100.0, 640.0);
    assert_eq!(status(&m), idle());

    assert!(cmd(&mut m, Command::SetScale { percent: 125 }).is_empty());
    assert_eq!(m.view().workspace.scale, 100);
    assert_eq!(
        status(&m),
        Some(StatusMessage::ScaleDoesNotFit {
            requested: 125,
            kept: 100,
        })
    );
    // Choosing the zoom that is on is nothing new, but it is something the user did.
    assert!(cmd(&mut m, Command::SetScale { percent: 100 }).is_empty());
    assert_eq!(status(&m), idle());
}

#[test]
fn a_new_scale_grows_a_window_that_is_too_small_for_it() {
    let mut m = playing();
    viewport(&mut m, 1024.0, 900.0);
    assert_eq!(cmd(&mut m, Command::SetScale { percent: 125 }).len(), 1);
    let ws = m.view().workspace;
    // Only the width was too small: 1280x800 is the smallest window at 125 %.
    assert_eq!((ws.viewport_width, ws.viewport_height), (1280.0, 900.0));
    assert_eq!(
        status(&m),
        Some(StatusMessage::ScaleWindowGrown { scale: 125 })
    );
    // The screen then tells the size the window really got.
    viewport(&mut m, 1280.0, 900.0);
    assert_eq!(
        status(&m),
        Some(StatusMessage::ScaleWindowGrown { scale: 125 })
    );

    // Big enough already: no notice.
    let mut m = playing();
    assert_eq!(cmd(&mut m, Command::SetScale { percent: 125 }).len(), 1);
    assert_eq!(status(&m), idle());
    let ws = m.view().workspace;
    assert_eq!((ws.viewport_width, ws.viewport_height), (1280.0, 800.0));
}

#[test]
fn a_grown_window_is_laid_out_at_its_new_size() {
    // The smallest window at 100 % becomes the smallest window at 125 %: the same 640 px at
    // 100 %, so the heights stay as they were.
    let mut m = playing();
    viewport(&mut m, 1024.0, 640.0);
    toggle(&mut m, Band::History);
    let before = m.view().workspace.layout;
    assert_eq!(
        before.arrange_height,
        640.0 - 24.0 - 48.0 - 24.0 - 248.0 - 24.0 - 24.0
    );
    cmd(&mut m, Command::SetScale { percent: 125 });
    let ws = m.view().workspace;
    assert_eq!((ws.viewport_width, ws.viewport_height), (1280.0, 800.0));
    assert_eq!(ws.layout, before);
}

#[test]
fn a_higher_scale_that_folds_a_band_says_the_scale_did_it() {
    // 800 CSS px is 640 at 125 %: the piano roll and the history need 676.
    let mut m = playing();
    toggle(&mut m, Band::PianoRoll);
    toggle(&mut m, Band::History);
    assert_eq!(open_bands(&m), [Band::PianoRoll, Band::History]);
    cmd(&mut m, Command::SetScale { percent: 125 });
    assert_eq!(open_bands(&m), [Band::History]);
    let folded = Some(StatusMessage::BandFoldedScale {
        scale: 125,
        band: Band::PianoRoll,
    });
    assert_eq!(status(&m), folded);
    // Resizing afterwards leaves it, like the other notices.
    viewport(&mut m, 1280.0, 900.0);
    assert_eq!(status(&m), folded);

    // The fold is told even when the window also grew.
    let mut m = playing();
    viewport(&mut m, 1024.0, 724.0);
    toggle(&mut m, Band::PianoRoll);
    toggle(&mut m, Band::History);
    cmd(&mut m, Command::SetScale { percent: 125 });
    assert_eq!(m.view().workspace.viewport_width, 1280.0);
    assert_eq!(
        status(&m),
        Some(StatusMessage::BandFoldedScale {
            scale: 125,
            band: Band::PianoRoll
        })
    );

    // Back to 100 %: nothing opens again.
    cmd(&mut m, Command::SetScale { percent: 100 });
    assert_eq!(open_bands(&m), [Band::History]);
    assert_eq!(status(&m), idle());
}

#[test]
fn a_smaller_screen_lowers_the_scale_and_saves_it() {
    let mut m = Model::new(Settings {
        output: None,
        output_name: None,
        scale: 125,
        ..Settings::default()
    });
    // The remembered zoom fits this screen.
    assert!(screen(&mut m, 1920.0, 1040.0).is_empty());
    assert_eq!(m.view().workspace.scale, 125);
    assert_eq!(status(&m), Some(StatusMessage::SearchingKeyboard));

    // A screen too small for 100 % still gets 100 %.
    assert_eq!(screen(&mut m, 800.0, 400.0), [saved(100)]);
    let ws = m.view().workspace;
    assert_eq!(ws.scale, 100);
    assert_eq!(ws.unfit_scales, [125]);
    let lowered = Some(StatusMessage::ScaleLoweredForScreen { from: 125, to: 100 });
    assert_eq!(status(&m), lowered);
    // The same screen again changes nothing.
    assert!(screen(&mut m, 800.0, 400.0).is_empty());
    assert_eq!(status(&m), lowered);
    assert!(screen(&mut m, 700.0, 400.0).is_empty());

    // A bigger screen does not raise the zoom again.
    assert!(screen(&mut m, 1920.0, 1040.0).is_empty());
    assert_eq!(m.view().workspace.scale, 100);
}

#[test]
fn retry_delay_doubles_from_half_a_second_and_stays_at_five() {
    let ms: Vec<u128> = (1..=8).map(|a| retry_delay(a).as_millis()).collect();
    assert_eq!(ms, [500, 1000, 2000, 4000, 5000, 5000, 5000, 5000]);
    assert_eq!(retry_delay(0).as_millis(), 500);
    assert_eq!(retry_delay(u32::MAX).as_millis(), 5000);
}

#[test]
fn pipewire_lost_closes_says_so_and_opens_again_with_the_next_picture() {
    let mut m = playing();
    m.handle(Input::AudioFrames(Some(256)));
    let song_before = m.view().song;

    assert_eq!(m.handle(Input::AudioSystemLost), [Effect::CloseOutput]);
    let view = m.view();
    assert_eq!(view.status.message, Some(StatusMessage::AudioSystemStopped));
    assert!(!view.audio.running);
    assert_eq!(view.song, song_before, "the song stays as it was");
    // The list stays as last seen, so the chosen output is not said to be missing.
    assert_eq!(view.outputs.chosen_state, Some(ChosenState::Ready));

    // The stream's own report comes after it was closed: nothing more to do.
    assert!(m.handle(Input::AudioLost).is_empty());
    // A user action does not hide it: it is not a notice.
    cmd(&mut m, Command::GoArrange);
    assert_eq!(status(&m), Some(StatusMessage::AudioSystemStopped));

    assert_eq!(m.handle(usual()), [open(HP_NODE)]);
    opened(&mut m, HP_NODE, 48_000);
    assert_eq!(status(&m), idle());
    assert!(m.view().audio.running);
}

#[test]
fn a_stream_that_ends_by_itself_is_opened_again_a_few_times() {
    let mut m = playing();
    m.handle(Input::AudioFrames(Some(256)));

    let mut waits = Vec::new();
    for _ in 0..4 {
        let effects = m.handle(Input::AudioLost);
        let [Effect::RetryAudio { after }, Effect::CloseOutput] = effects[..] else {
            panic!("{effects:?}");
        };
        waits.push(after.as_millis());
        assert!(!m.view().audio.running);
        assert_eq!(m.handle(Input::AudioRetry), [open(HP_NODE)]);
        opened(&mut m, HP_NODE, 48_000);
    }
    assert_eq!(waits, [500, 1000, 2000, 4000]);
    // Nothing is said while it tries again.
    assert_eq!(status(&m), idle());
    // Enough: it waits for the outputs to change, and says the output went away.
    assert_eq!(m.handle(Input::AudioLost), [Effect::CloseOutput]);
    assert!(m.handle(Input::AudioRetry).is_empty());
    let missing = Some(StatusMessage::OutputMissing {
        output: "アナログヘッドフォン".into(),
    });
    assert_eq!(status(&m), missing);
    // It is not a notice: the next thing the user does leaves it.
    cmd(&mut m, Command::SelectClip { clip: None });
    assert_eq!(status(&m), missing);
    assert_eq!(m.handle(usual()), [open(HP_NODE)]);
    assert_eq!(status(&m), idle());
    opened(&mut m, HP_NODE, 48_000);

    // Once a stream runs, the count starts over.
    m.handle(Input::AudioFrames(Some(256)));
    let effects = m.handle(Input::AudioLost);
    assert!(
        matches!(effects[..], [Effect::RetryAudio { after }, Effect::CloseOutput] if after.as_millis() == 500)
    );

    // A retry that fails to open waits again.
    let effects = m.handle(Input::AudioRetry);
    assert_eq!(effects, [open(HP_NODE)]);
    let effects = m.handle(Input::AudioOpenFailed {
        node_name: HP_NODE.into(),
    });
    assert!(matches!(effects[..], [Effect::RetryAudio { after }] if after.as_millis() == 1000));
}

#[test]
fn a_stream_that_fails_to_open_again_four_times_says_the_output_went_away() {
    let mut m = playing();
    m.handle(Input::AudioFrames(Some(256)));
    m.handle(Input::AudioLost);
    for _ in 0..4 {
        assert_eq!(status(&m), idle());
        assert_eq!(m.handle(Input::AudioRetry), [open(HP_NODE)]);
        m.handle(Input::AudioOpenFailed {
            node_name: HP_NODE.into(),
        });
    }
    let missing = Some(StatusMessage::OutputMissing {
        output: "アナログヘッドフォン".into(),
    });
    assert_eq!(status(&m), missing);
    assert!(!m.view().audio.running);
    // A test tone on it in the settings tries again and takes the sentence away.
    cmd(&mut m, Command::OpenSettings { advanced: false });
    cmd(&mut m, Command::TestTone { output: HP.into() });
    cmd(&mut m, Command::SettingsCancel);
    assert_ne!(status(&m), missing);
}

#[test]
fn a_refused_value_is_said_until_the_next_thing_the_user_does() {
    use meloet_protocol::ValueField;
    let mut m = playing();
    let refused = Some(StatusMessage::ValueRejected {
        field: ValueField::Tempo,
        kept: "96.00".into(),
        range: None,
    });
    let effects = cmd(
        &mut m,
        Command::RejectValue {
            field: ValueField::Tempo,
            kept: "96.00".into(),
            range: None,
        },
    );
    assert!(effects.is_empty(), "{effects:?}");
    assert_eq!(status(&m), refused);
    viewport(&mut m, 1280.0, 900.0);
    assert_eq!(status(&m), refused);
    cmd(&mut m, Command::SelectClip { clip: None });
    assert_eq!(status(&m), idle());
}

#[test]
fn audio_lost_without_an_open_stream_does_nothing() {
    let mut m = fresh();
    m.handle(usual());
    assert!(m.handle(Input::AudioLost).is_empty());
    assert!(m.handle(Input::AudioRetry).is_empty());
}

#[test]
fn xruns_are_kept_for_the_open_stream_only() {
    let mut m = playing();
    m.handle(Input::AudioXruns { count: 3 });
    assert_eq!(m.xruns(), 3);
    m.handle(Input::AudioSystemLost);
    assert_eq!(m.xruns(), 0);
    m.handle(Input::AudioXruns { count: 5 });
    assert_eq!(m.xruns(), 0, "no stream is open");
}

#[test]
fn midi_stopped_releases_the_keys_says_so_and_ends_with_midi_back() {
    let mut m = playing();
    m.handle(Input::KeyboardAdded(keyboard(2)));
    m.handle(Input::KeyboardPlayed { id: "kb1".into() });

    assert_eq!(m.handle(Input::MidiStopped), [Effect::AllNotesOff]);
    let view = m.view();
    assert_eq!(
        view.status.message,
        Some(StatusMessage::KeyboardInputStopped)
    );
    assert!(view.keyboards.input_stopped);
    assert_eq!(
        view.keyboards.list.len(),
        2,
        "the list stays while it starts again"
    );
    // A second report while it is already stopped releases nothing new.
    assert!(m.handle(Input::MidiStopped).is_empty());

    // The new thread reports what is there; kb2 went away meanwhile.
    m.handle(Input::KeyboardAdded(keyboard(1)));
    assert!(m.handle(Input::MidiBack).is_empty());
    let view = m.view();
    assert!(!view.keyboards.input_stopped);
    assert_eq!(status(&m), idle());
    let ids: Vec<&str> = view.keyboards.list.iter().map(|k| k.id.as_str()).collect();
    assert_eq!(ids, ["kb1"]);
    assert_eq!(view.keyboards.active.as_deref(), Some("kb1"));
    assert_eq!(view.keyboards.lost, None);
}

#[test]
fn midi_back_with_no_keyboard_left_says_the_last_one_is_lost() {
    let mut m = playing();
    m.handle(Input::MidiStopped);
    m.handle(Input::MidiBack);
    let view = m.view();
    assert!(view.keyboards.list.is_empty());
    assert_eq!(view.keyboards.lost.as_deref(), Some("Keystation 1"));
    assert!(matches!(
        status(&m),
        Some(StatusMessage::KeyboardLost { .. })
    ));
    // The first Ready after start is not a restart.
    let mut m = fresh();
    m.handle(Input::KeyboardAdded(keyboard(1)));
    assert!(m.handle(Input::MidiBack).is_empty());
    assert_eq!(m.view().keyboards.list.len(), 1);
}

#[test]
fn status_order_of_the_stopped_systems() {
    use StatusMessage::*;
    let mut m = playing();
    m.handle(Input::MidiStopped);
    assert_eq!(status(&m), Some(KeyboardInputStopped));
    m.handle(picture(vec![headphones(Av::No, true)], None));
    assert!(matches!(status(&m), Some(OutputMissing { .. })));
    m.handle(Input::AudioSystemLost);
    assert_eq!(status(&m), Some(AudioSystemStopped));
    cmd(&mut m, Command::OpenSettings { advanced: false });
    assert_eq!(status(&m), Some(SettingsOpen));
}

#[test]
fn no_pipewire_at_start_says_nothing_about_it_and_the_first_picture_goes_on_as_usual() {
    let mut m = remembering(HP);
    m.handle(Input::KeyboardAdded(keyboard(1)));
    let view = m.view();
    assert_ne!(view.status.message, Some(StatusMessage::AudioSystemStopped));
    assert!(view.outputs.list.is_empty());
    assert_eq!(view.outputs.chosen_state, None);
    assert!(!view.audio.running);

    // PipeWire comes up later: the remembered output opens.
    assert_eq!(m.handle(usual()), [open(HP_NODE)]);
    opened(&mut m, HP_NODE, 48_000);
    assert_eq!(status(&m), Some(new_song()));

    // Without a remembered output, the chooser comes up.
    let mut m = fresh();
    m.handle(Input::KeyboardAdded(keyboard(1)));
    assert!(m.view().workspace.chooser.is_some());
    assert!(m.handle(usual()).is_empty());
    assert_eq!(
        m.view().workspace.chooser.unwrap().highlighted.as_deref(),
        Some(HP)
    );
}

#[test]
fn the_shelf_starts_closed_with_the_two_favorites_and_a_star_is_a_setting() {
    let mut m = fresh();
    let shelf = m.view().shelf;
    assert_eq!(
        shelf,
        ShelfView {
            patterns: crate::patterns::all().iter().map(|p| p.view()).collect(),
            selected_pattern: None,
            pattern_sample: None,
            mode: ShelfMode::NewTrack,
            available: vec![InstrumentKind::Sine],
            selected_sound: None,
            current_sound: None,
            loading_sound: None,
            sound_sample: None,
            chosen_effect: None,
            current_effect: None,
            unreadable: None,
            favorite_sounds: [InstrumentKind::GrandPiano, InstrumentKind::StudioKit]
                .map(|instrument| SoundRef::Builtin { instrument })
                .to_vec(),
            favorite_effects: [EffectKind::Reverb, EffectKind::Delay]
                .map(|effect| EffectRef::Builtin { effect })
                .to_vec(),
            missing: vec![],
            not_yet: vec![],
        }
    );
    let star = |m: &mut Model, instrument| {
        let effects = cmd(m, Command::ToggleFavorite { instrument });
        let saved = effects.iter().find_map(|e| match e {
            Effect::SaveSettings(s) => Some(s.favorites.clone()),
            _ => None,
        });
        (m.view().shelf.favorites(), saved)
    };
    // The view keeps the order of the shelf, whatever the order of the clicks.
    let (shown, saved) = star(&mut m, InstrumentKind::Organ);
    assert_eq!(
        shown,
        [
            InstrumentKind::GrandPiano,
            InstrumentKind::Organ,
            InstrumentKind::StudioKit
        ]
    );
    assert_eq!(saved.map(|s| s.len()), Some(3));
    let (shown, saved) = star(&mut m, InstrumentKind::GrandPiano);
    assert_eq!(shown, [InstrumentKind::Organ, InstrumentKind::StudioKit]);
    assert_eq!(
        saved,
        Some(vec![BuiltinInstrument::StudioKit, BuiltinInstrument::Organ])
    );
    assert_eq!(m.view().song.undo_count, 0, "not part of the song");
}

#[test]
fn the_shelf_commands_for_any_sound_do_what_the_ones_for_meloet_s_sounds_do() {
    use meloet_protocol::MissingRef;
    let mut m = fresh();
    cmd(
        &mut m,
        Command::OpenShelf {
            mode: ShelfMode::NewTrack,
        },
    );
    let bass = SoundRef::Builtin {
        instrument: InstrumentKind::SynthBass,
    };
    let sound = Some(bass.clone());
    cmd(&mut m, Command::ShelfSelectSound { sound });
    let shelf = m.view().shelf;
    assert_eq!(shelf.selected(), Some(InstrumentKind::SynthBass));
    assert_eq!(shelf.selected_sound, Some(bass.clone()));
    // The shelf has no plugin: selecting one ends the audition, as a sound it does not have.
    let surge = SoundRef::Plugin {
        key: "lv2:https://surge-synthesizer.github.io/lv2/surge-xt".into(),
    };
    let sound = Some(surge.clone());
    cmd(&mut m, Command::ShelfSelectSound { sound });
    assert_eq!(m.view().shelf.selected_sound, None);
    let sound = bass.clone();
    cmd(&mut m, Command::ToggleFavoriteSound { sound });
    assert!(m.view().shelf.favorite_sounds.contains(&bass));
    assert!(
        m.view()
            .shelf
            .favorites()
            .contains(&InstrumentKind::SynthBass)
    );
    // What the shelf does not have, and what is not made yet, changes nothing.
    let before = m.view();
    for command in [
        Command::ToggleFavoriteSound {
            sound: surge.clone(),
        },
        Command::PlaySampleOf { sound: surge },
        Command::ToggleFavoriteEffect {
            effect: EffectRef::Plugin {
                key: "lv2:urn:not:there".into(),
            },
        },
        Command::ShelfChooseEffect { effect: None },
        Command::UsePluginFormat {
            key: "vst3:00".into(),
        },
        Command::OpenPluginWindow { slot: None },
        Command::CancelFileDrop,
        Command::RemoveMissing {
            missing: MissingRef::Sf2 {
                file: "Old Keys.sf2".into(),
            },
        },
    ] {
        assert!(cmd(&mut m, command.clone()).is_empty(), "{command:?}");
        assert_eq!(m.view(), before, "{command:?}");
    }
}

/// A song that uses a plugin and an SF2 preset opens on a PC that has neither.
#[test]
fn a_song_with_what_this_pc_does_not_have_keeps_it_and_says_so() {
    use std::sync::Arc;

    use meloet_model::{InstrumentRef, Kept, PluginRef, Sf2Ref, edit, mixer};
    use meloet_protocol::{
        GmFamily, MissingView, PluginFormat, PluginRole, PluginState, PluginView,
    };
    let mut song = Song::new_default();
    let surge = PluginRef::new(
        "vst3:00112233445566778899AABBCCDDEEFF",
        "Surge XT",
        "Surge",
        "",
    );
    let kit = Sf2Ref {
        file: "FluidR3_GM.sf2".into(),
        sha256: "".into(),
        bank: 128,
        program: 0,
        name: "Standard".into(),
        kept: Kept::NONE,
    };
    for instrument in [
        InstrumentRef::Plugin(Arc::new(surge.clone())),
        InstrumentRef::Sf2(Arc::new(kit)),
    ] {
        song.doc = edit::add_track(&song.doc, None, instrument, meloet_model::Language::Ja)
            .unwrap()
            .0
            .doc;
    }
    let strip = meloet_model::StripId::Track(song.doc.tracks[0].id);
    let mverb = PluginRef::new("lv2:urn:mverb", "MVerb", "DISTRHO", "");
    song.doc = mixer::add_plugin_effect(&song.doc, strip, 1, Arc::new(mverb))
        .unwrap()
        .doc;
    let mut m = fresh();
    cmd(&mut m, Command::OpenSong);
    let path = std::path::PathBuf::from("/nowhere/曲/曲.meloet");
    m.handle(Input::SongFilePicked(Some(path.clone())));
    m.handle(Input::SongRead {
        path,
        song: Box::new(song.clone()),
        from_previous: false,
    });
    let view = m.view();
    let tracks = &view.song.tracks;
    assert_eq!(
        tracks.iter().map(|t| &*t.name).collect::<Vec<_>>(),
        ["ピアノ", "Surge XT", "ドラム"]
    );
    let surge_view = PluginView {
        key: surge.key.to_string(),
        name: "Surge XT".into(),
        format: Some(PluginFormat::Vst3),
        state: PluginState::Missing,
    };
    assert_eq!(tracks[1].sound, SoundView::Plugin(surge_view));
    assert_eq!(tracks[1].instrument(), InstrumentKind::Sine);
    assert_eq!(tracks[1].stand_in, Some(StandIn::PluginMissing));
    assert_eq!(
        tracks[2].sound,
        SoundView::Sf2 {
            file: "FluidR3_GM.sf2".into(),
            bank: 128,
            program: 0,
            name: "Standard".into(),
            family: GmFamily::DrumKit,
        }
    );
    assert_eq!(tracks[2].stand_in, Some(StandIn::Sf2Missing));
    assert_eq!(tracks[2].drum_keys, None);
    assert_eq!(
        tracks[0].inserts(),
        [None; 4],
        "a plugin is no effect of meloet"
    );
    let slot = tracks[0].slots[1].as_ref().unwrap();
    assert!(!slot.bypassed);
    assert!(matches!(
        &slot.effect,
        meloet_protocol::InsertEffectView::Plugin(p)
            if p.name == "MVerb" && p.state == PluginState::Missing
    ));
    assert_eq!(
        view.shelf.missing,
        [
            MissingView::Sf2 {
                file: "FluidR3_GM.sf2".into(),
                tracks: 1
            },
            // In the order of the tracks: MVerb sits on the first one.
            MissingView::Plugin {
                key: "lv2:urn:mverb".into(),
                name: "MVerb".into(),
                vendor: "DISTRHO".into(),
                format: Some(PluginFormat::Lv2),
                role: PluginRole::Effect,
                shown_path: None,
            },
            MissingView::Plugin {
                key: surge.key.to_string(),
                name: "Surge XT".into(),
                vendor: "Surge".into(),
                format: Some(PluginFormat::Vst3),
                role: PluginRole::Instrument,
                shown_path: None,
            },
        ]
    );
    // The song itself is as it was: saving writes the plugin and the preset back.
    assert_eq!(m.play().song().doc, song.doc);
}

/// 「使い方をひらく」 asks the shell to open the guide; the state does not change.
#[test]
fn the_user_guide_opens_in_the_browser_and_the_state_stays() {
    let mut m = fresh();
    let before = m.view();
    assert_eq!(
        cmd(&mut m, Command::OpenUserGuide),
        [Effect::OpenUrl(super::USER_GUIDE_URL.to_string())]
    );
    assert!(super::USER_GUIDE_URL.starts_with("https://"));
    assert_eq!(AppState::changes_from(&before, &m.view()), []);
    // In English, the English guide.
    let mut m = fresh().with_pc_language(meloet_protocol::Language::En);
    assert_eq!(
        cmd(&mut m, Command::OpenUserGuide),
        [Effect::OpenUrl(super::USER_GUIDE_URL_EN.to_string())]
    );
}

/// Protocol 32 (round 12, wave 2): every new intent is taken, and until its feature is built
/// it changes nothing, as an intent with an old id.
#[test]
fn the_commands_of_round_12_wave_2_are_taken_and_change_nothing_yet() {
    use meloet_protocol::PresetsView;
    let mut m = fresh();
    let before = m.view();
    assert!(!before.transport.can_keep_played);
    assert!(!before.keyboards.typed_keys);
    assert!(before.workspace.note_glow);
    assert_eq!(before.presets, PresetsView::default());
    assert_eq!(
        (
            before.shelf.patterns.len(),
            &before.shelf.selected_pattern,
            &before.shelf.pattern_sample
        ),
        (12, &None, &None),
        "the patterns of the shelf of a new track (P4)"
    );
    let commands = [
        // Played by the service; nothing for the rules.
        Command::TypedKey {
            note: 60,
            down: true,
            t_us: 1_000,
        },
        Command::ShelfSelectPattern {
            id: Some("eightBeat".into()),
        },
        Command::PlayPattern {
            id: "eightBeat".into(),
        },
        Command::PlacePattern {
            id: "eightBeat".into(),
        },
        Command::RepeatClips {
            clips: vec![99],
            count: 3,
        },
    ];
    for command in commands {
        assert_eq!(cmd(&mut m, command.clone()), [], "{command:?}");
    }
    assert_eq!(AppState::changes_from(&before, &m.view()), []);

    // The settings are read and written back as they are.
    let settings = Settings {
        note_glow: false,
        export_artist: "わたし".into(),
        ..Settings::default()
    };
    let mut m = Model::new(settings.clone());
    assert!(!m.view().workspace.note_glow);
    cmd(&mut m, Command::OpenExport);
    assert_eq!(m.view().export.dialog.expect("the dialog").artist, "わたし");
    assert_eq!(*m.settings(), settings);
}

/// Protocol 31 (round 12, wave 1): every new intent is taken, and until its feature is built
/// it changes nothing, as an intent with an old id.
#[test]
fn the_commands_of_round_12_are_taken_and_change_nothing_yet() {
    use meloet_protocol::{ClickSound, ClickWhen};
    let mut m = fresh();
    let before = m.view();
    assert_eq!(
        (
            before.transport.click_volume_db,
            before.transport.click_sound,
            before.transport.click_when
        ),
        (0.0, ClickSound::Beep, ClickWhen::Always)
    );
    assert!(before.recent.is_empty());
    assert_eq!(before.updates, UpdatesView::default());
    let commands = [
        Command::ScaleVelocity {
            take: 99,
            notes: vec![0],
            percent: 120,
        },
        Command::RampVelocity {
            take: 99,
            notes: vec![0, 1],
            from: 40,
            to: 110,
        },
        Command::RandomVelocity {
            take: 99,
            notes: vec![0],
            amount: 8,
            seed: 7,
        },
        Command::Legato {
            take: 99,
            notes: vec![0, 1],
        },
        Command::PlayExported,
        Command::OpenRecent { id: 1 },
        Command::ClearRecent,
        Command::NormalizeClips { clips: vec![1] },
        Command::ReverseClips { clips: vec![1] },
        Command::OpenReleasePage,
        Command::DismissNewVersion,
    ];
    for command in commands {
        assert_eq!(cmd(&mut m, command.clone()), [], "{command:?}");
    }
    assert_eq!(AppState::changes_from(&before, &m.view()), []);

    // The settings are read and written back as they are.
    let settings = Settings {
        click: crate::settings::ClickSettings {
            volume_db: -12.0,
            sound: ClickSound::Cowbell,
            when: ClickWhen::RecordingOnly,
        },
        check_updates: false,
        ..Settings::default()
    };
    let m = Model::new(settings.clone());
    let view = m.view();
    assert_eq!(
        (
            view.transport.click_volume_db,
            view.transport.click_sound,
            view.transport.click_when,
            view.updates.check
        ),
        (-12.0, ClickSound::Cowbell, ClickWhen::RecordingOnly, false)
    );
    assert_eq!(*m.settings(), settings);
}
