use meloet_devices::{Availability as Av, ClockSettings, Friendly as Fr, LinkKind as Link};

use super::*;

const HP: &str = "card:analog-output-headphones";
const HP_NODE: &str = "alsa_output.card.analog-stereo";
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
    assert!(effects.is_empty());
}

fn status(model: &Model) -> Option<StatusMessage> {
    model.view().status.message
}

/// A model that chose the headphones through the chooser and has its stream open at 48 kHz.
fn playing() -> Model {
    let mut m = fresh();
    m.handle(Input::KeyboardAdded(keyboard(1)));
    m.handle(usual());
    cmd(&mut m, Command::ChooserConfirm);
    opened(&mut m, HP_NODE, 48_000);
    m
}

#[test]
fn a_new_song() {
    let state = fresh().view();
    let song = &state.song;
    assert_eq!(
        (song.tempo_changes, song.undo_count, song.note_count),
        (0, 0, 0)
    );
    assert_eq!(song.play_target, Some(1));
    assert_eq!(
        song.tracks,
        [TrackView {
            number: 1,
            name: "ピアノ".into(),
            color: 1,
            instrument: InstrumentKind::Placeholder,
            gain_db: 0.0,
        }]
    );
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
            Effect::SaveSettings(Settings {
                output: Some(USB.into()),
                output_name: Some("USB スピーカー".into()),
                scale: 100,
            }),
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
        assert_eq!(status(&m), None);
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
    assert_eq!(status(&m), None);

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
            Effect::SaveSettings(Settings {
                output: Some(USB.into()),
                output_name: Some("USB スピーカー".into()),
                scale: 100,
            }),
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
    assert_eq!(view.status.message, None);
    assert_eq!(view.audio.actual_frames, Some(128));
    assert_eq!(view.audio.forced_quantum, None);
    assert!((view.audio.latency_ms.unwrap() - 6.67).abs() < 0.01);

    // Forced to a size other than the one running.
    m.handle(picture(outputs(), Some(1024)));
    assert_eq!(status(&m), None);

    // Forced to the size that was asked for.
    m.handle(picture(outputs(), Some(256)));
    m.handle(Input::AudioFrames(Some(256)));
    assert_eq!(status(&m), None);
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
    assert_eq!(status(&m), None, "OutputChosen and NewSong are not said");

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
    assert_eq!(hint(&m), None);
    m.handle(Input::SoundHeard);
    assert_eq!(hint(&m), Some(Hint::SoundCameOut));
    assert!(!m.wants_sound());

    cmd(&mut m, Command::DismissHint);
    assert_eq!(hint(&m), None);
    m.handle(Input::SoundHeard);
    assert_eq!(hint(&m), None);
    assert!(!m.wants_sound());
}

#[test]
fn an_output_remembered_from_before_gives_no_hint() {
    let mut m = remembering(HP);
    m.handle(Input::KeyboardAdded(keyboard(1)));
    m.handle(usual());
    opened(&mut m, HP_NODE, 48_000);
    m.handle(Input::SoundHeard);
    assert_eq!(m.view().workspace.hint, None);

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
    assert_eq!(l.arrange_height, 800.0 - 48.0 - 52.0 - 24.0 - 24.0);
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
            height: 800.0 - 48.0 - 24.0 - 40.0 - 24.0 - 24.0
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
    // 1440 CSS px at 150 % is 960 at 100 %: the tall-window rule keeps the meter full.
    let effects = cmd(&mut m, Command::SetScale { percent: 150 });
    assert_eq!(
        effects,
        [Effect::SaveSettings(Settings {
            output: None,
            output_name: None,
            scale: 150
        })]
    );
    cmd(
        &mut m,
        Command::SetViewport {
            width: 1920.0,
            height: 1440.0,
        },
    );
    cmd(&mut m, Command::ToggleBand { band: Band::Mixer });
    let ws = m.view().workspace;
    assert_eq!(
        (ws.scale, ws.viewport_width, ws.viewport_height),
        (150, 1920.0, 1440.0)
    );
    assert_eq!(ws.layout.meter, MeterSize::Full);
    assert_eq!(
        ws.layout.arrange_height,
        960.0 - 48.0 - 52.0 - 516.0 - 24.0 - 24.0
    );

    cmd(
        &mut m,
        Command::SetViewport {
            width: 1920.0,
            height: 1080.0,
        },
    );
    let l = m.view().workspace.layout;
    assert_eq!(l.meter, MeterSize::Slim);
    assert_eq!(l.arrange_height, 40.0);
    assert_eq!(l.lower[0].height, 720.0 - 48.0 - 24.0 - 40.0 - 24.0 - 24.0);

    // Other scales and broken sizes are ignored.
    for percent in [0, 99, 110, 200] {
        assert!(cmd(&mut m, Command::SetScale { percent }).is_empty());
    }
    assert!(cmd(&mut m, Command::SetScale { percent: 150 }).is_empty());
    for (width, height) in [(0.0, 800.0), (1280.0, -1.0), (f32::NAN, 800.0)] {
        cmd(&mut m, Command::SetViewport { width, height });
    }
    let ws = m.view().workspace;
    assert_eq!(
        (ws.scale, ws.viewport_width, ws.viewport_height),
        (150, 1920.0, 1080.0)
    );

    assert_eq!(
        cmd(&mut m, Command::SetScale { percent: 125 }).len(),
        1,
        "saved"
    );
    assert_eq!(m.view().workspace.scale, 125);
}

#[test]
fn arrange_no_taller_than_the_overview_is_drawn_as_the_overview() {
    let mut m = fresh();
    // History 248 + 40 with the slim meter (24 + 48 + 24 + 24 = 120).
    cmd(
        &mut m,
        Command::SetViewport {
            width: 1280.0,
            height: 408.0,
        },
    );
    cmd(
        &mut m,
        Command::ToggleBand {
            band: Band::History,
        },
    );
    let l = m.view().workspace.layout;
    assert_eq!(l.arrange_height, 40.0);
    assert!(l.arrange_overview);

    cmd(
        &mut m,
        Command::SetViewport {
            width: 1280.0,
            height: 409.0,
        },
    );
    let l = m.view().workspace.layout;
    assert_eq!(l.arrange_height, 41.0);
    assert!(!l.arrange_overview);
}

#[test]
fn the_scale_comes_from_the_settings() {
    let m = Model::new(Settings {
        output: None,
        output_name: None,
        scale: 125,
    });
    let ws = m.view().workspace;
    assert_eq!(ws.scale, 125);
    assert_eq!(
        ws.layout.arrange_height,
        800.0 / 1.25 - 48.0 - 52.0 - 24.0 - 24.0
    );
}
