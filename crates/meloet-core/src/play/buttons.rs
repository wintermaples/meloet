//! What the buttons and the fader of a keyboard need to know of the song
//! (`crate::state::buttons`).

use meloet_model::TrackId;
use meloet_model::mixer::{Gain, StripId};
use meloet_protocol::TransportState;

use super::Play;

impl Play {
    pub fn is_stopped(&self) -> bool {
        self.state == TransportState::Stopped
    }

    /// The head of the bar one bar before the play line (`forward`: after it). A play line
    /// inside a bar goes back to that bar's head. Never before the song's head.
    pub fn bar_step(&self, forward: bool) -> u64 {
        let map = self.map();
        let line = self.play_line();
        let at = map.meter_at(line);
        if forward {
            map.tick_of_bar(at.bar + 1)
        } else if at.bar_tick < line {
            at.bar_tick
        } else {
            at.bar.checked_sub(1).map_or(0, |bar| map.tick_of_bar(bar))
        }
    }

    /// The track above the selected one (`down`: below it), `None` at the end of the list.
    /// With no track selected, the first track.
    pub fn track_beside(&self, down: bool) -> Option<u32> {
        let tracks = &self.doc().tracks;
        let Some(selected) = self.song.session.play_target else {
            return tracks.first().map(|t| t.id.0);
        };
        let i = tracks.iter().position(|t| t.id == selected)?;
        let j = if down {
            i.checked_add(1)?
        } else {
            i.checked_sub(1)?
        };
        tracks.get(j).map(|t| t.id.0)
    }

    /// The selected track and the level its fader is at in the song.
    pub fn selected_gain(&self) -> Option<(u32, Gain)> {
        let id: TrackId = self.song.session.play_target?;
        let channel = self.doc().channel(StripId::Track(id))?;
        Some((id.0, channel.gain))
    }
}
