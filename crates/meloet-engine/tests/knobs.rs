//! The knobs of a sound that comes with meloet (round 12, E3) as the engine hands them over:
//! the hand's values kept for the seat, a lane that plays a knob in their place, the one
//! tried out on the seat left alone, and an instrument put on the seat taking them.

use meloet_engine::{
    AutoLane, BlockTiming, Engine, EngineCommand, EngineConfig, EngineParts, Instrument,
    InstrumentEvent, KNOB_DEFAULTS, KNOBS,
};
use meloet_model::AutoTarget;

const BLOCK: usize = 256;

/// Sounds its 「明るさ」 on the left and its 「立ち上がり」 on the right, as they were set.
struct Shows {
    knobs: [f32; KNOBS],
}

impl Shows {
    fn new() -> Box<Shows> {
        Box::new(Shows {
            knobs: KNOB_DEFAULTS,
        })
    }
}

impl Instrument for Shows {
    fn prepare(&mut self, _sample_rate: f32, _max_frames: usize) {}
    fn event(&mut self, _ev: InstrumentEvent) {}
    fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
        left.fill(self.knobs[0]);
        right.fill(self.knobs[1]);
    }
    fn reset(&mut self) {}
    fn set_param_at(&mut self, _frame: u32, id: u32, value: f32) {
        self.set_knob(id, value);
    }
    fn set_knob(&mut self, knob: u32, place: f32) {
        if let Some(k) = self.knobs.get_mut(knob as usize) {
            *k = place;
        }
    }
}

struct Rig {
    parts: EngineParts,
    out: Vec<f32>,
}

impl Rig {
    fn new() -> Rig {
        let mut parts = Engine::new(EngineConfig::default());
        assert!(parts.handle.swap_instrument(0, Shows::new()).is_ok());
        Rig {
            parts,
            out: vec![0.0; 2 * BLOCK],
        }
    }

    fn send(&mut self, command: EngineCommand) {
        assert!(self.parts.handle.send(command).is_ok(), "command ring full");
    }

    /// Left and right at the end of a few blocks.
    fn heard(&mut self) -> (f32, f32) {
        for _ in 0..8 {
            self.parts
                .engine
                .process(&mut self.out, BlockTiming::default());
            self.parts.handle.drop_retired();
        }
        let last = &self.out[2 * BLOCK - 2..];
        (last[0], last[1])
    }

    fn knobs(&mut self, brightness: f32, attack: f32) {
        self.send(EngineCommand::SetKnobs {
            track: 0,
            knobs: [brightness, attack, 0.5, 0.0],
        });
    }

    fn lane(&mut self, param: u32, place: f32) {
        let lane = AutoLane {
            target: AutoTarget::Instrument { param },
            step: false,
            points: vec![(0, place)],
            slot: 0,
            effect: None,
        };
        assert!(self.parts.handle.set_automation(0, vec![lane]).is_ok());
    }
}

fn close(heard: (f32, f32), want: (f32, f32)) -> bool {
    (heard.0 - want.0).abs() < 1e-6 && (heard.1 - want.1).abs() < 1e-6
}

#[test]
fn the_hands_values_and_the_lanes_take_turns() {
    let mut rig = Rig::new();
    assert!(close(rig.heard(), (0.5, 0.5)), "where the knobs start");
    rig.knobs(0.2, 0.7);
    assert!(close(rig.heard(), (0.2, 0.7)));
    rig.lane(0, 0.9);
    assert!(close(rig.heard(), (0.9, 0.7)), "the lane plays 「明るさ」");
    rig.knobs(0.3, 0.6);
    assert!(
        close(rig.heard(), (0.9, 0.6)),
        "a knob a lane plays keeps the lane's value"
    );
    let hold = |on| EngineCommand::HoldLane {
        track: 0,
        target: AutoTarget::Instrument { param: 0 },
        on,
    };
    rig.send(hold(true));
    assert!(
        close(rig.heard(), (0.3, 0.6)),
        "a held lane gives the hand's value"
    );
    rig.send(hold(false));
    assert!(
        close(rig.heard(), (0.9, 0.6)),
        "and the lane's again when let go"
    );
    rig.send(EngineCommand::SetAutomation {
        track: 0,
        set: None,
    });
    assert!(close(rig.heard(), (0.3, 0.6)), "no lane: the hand's value");
}

#[test]
fn an_instrument_on_the_seat_takes_its_knobs_and_one_tried_out_does_not() {
    let mut rig = Rig::new();
    rig.knobs(0.1, 0.8);
    rig.lane(1, 0.25);
    assert!(close(rig.heard(), (0.1, 0.25)));
    assert!(rig.parts.handle.swap_instrument(0, Shows::new()).is_ok());
    assert!(
        close(rig.heard(), (0.1, 0.25)),
        "a new instrument: the hand and the lane"
    );

    assert!(rig.parts.handle.try_instrument(0, Shows::new()).is_ok());
    assert!(
        close(rig.heard(), (0.5, 0.5)),
        "the one tried out starts as it is"
    );
    rig.knobs(0.4, 0.8);
    assert!(
        close(rig.heard(), (0.5, 0.5)),
        "and the knobs of the track are not its"
    );
    rig.send(EngineCommand::EndTrial {
        track: 0,
        keep: false,
    });
    assert!(
        close(rig.heard(), (0.4, 0.25)),
        "the track's own is back, with its knobs"
    );

    rig.send(EngineCommand::ClearTrack { track: 0 });
    assert!(rig.parts.handle.swap_instrument(0, Shows::new()).is_ok());
    assert!(
        close(rig.heard(), (0.5, 0.5)),
        "an emptied seat starts again"
    );
}
