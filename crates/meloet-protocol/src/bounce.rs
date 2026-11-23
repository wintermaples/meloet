//! Turning keys into audio (protocol 33, round 12 A7): 「音のクリップにする」 and
//! 「このトラックを音にする」, made by a background job.

use serde::{Deserialize, Serialize};

use crate::TrackRef;

/// The job that makes the audio (`AudioWorkView::bounce`): the status line says 「音にしています…
/// 40 %」 with 「やめる」 (`Command::CancelBounce`). One at a time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct BounceView {
    /// The track it is made from, as it was when the job started (it may be gone by now).
    pub track: TrackRef,
    /// 0..=99.
    pub percent: u8,
}
