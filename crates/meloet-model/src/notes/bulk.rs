//! The tools of 「ベロシティをまとめて変える」 and 「長さをそろえる」 that work on the picked
//! notes together: by a percent, by a ramp, by a random spread, and 「次の音までのばす」.
//! Each is one step that keeps the picked notes selected; no change at all is
//! [`EditError::Unchanged`].

use super::*;

/// The percent of [`scale_velocity`].
pub const SCALE_PERCENT: std::ops::RangeInclusive<u16> = 10..=300;
/// The ± of [`random_velocity`].
pub const RANDOM_AMOUNT: std::ops::RangeInclusive<u8> = 1..=32;

/// The splitmix64 mix of `x`: the same `x` gives the same number.
pub fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn velocity(v: i64) -> u8 {
    v.clamp(1, 127) as u8
}

/// Each velocity becomes `round(v × percent / 100)`, put into 1..=127.
pub fn scale_velocity(
    doc: &Doc,
    clip: ClipId,
    indexes: &[usize],
    percent: u16,
) -> Result<NotesEdited, EditError> {
    let p = i64::from(percent.clamp(*SCALE_PERCENT.start(), *SCALE_PERCENT.end()));
    change_picked(
        doc,
        clip,
        indexes,
        |count| StepKind::ScaleVelocity { clip, count },
        |picked| {
            for (_, n) in picked {
                n.velocity = velocity((i64::from(n.velocity) * p + 50) / 100);
            }
        },
    )
}

/// From `from` at the earliest start of the picked notes to `to` at the latest, in a straight
/// line by start (rounded). All at one start: `from`.
pub fn ramp_velocity(
    doc: &Doc,
    clip: ClipId,
    indexes: &[usize],
    from: u8,
    to: u8,
) -> Result<NotesEdited, EditError> {
    let from = i128::from(from.clamp(1, 127));
    let to = i128::from(to.clamp(1, 127));
    change_picked(
        doc,
        clip,
        indexes,
        |count| StepKind::RampVelocity { clip, count },
        |picked| {
            let first = picked.iter().map(|(_, n)| n.start).min().unwrap_or(0);
            let last = picked.iter().map(|(_, n)| n.start).max().unwrap_or(0);
            let span = i128::from(last - first);
            for (_, n) in picked {
                let v = if span == 0 {
                    from
                } else {
                    let num = (to - from) * i128::from(n.start - first);
                    // Rounded, half away from zero.
                    let r = (2 * num.abs() + span) / (2 * span);
                    from + num.signum() * r
                };
                n.velocity = velocity(v as i64);
            }
        },
    )
}

/// Each picked note gets a whole number from −`amount` to +`amount` added, put into 1..=127.
/// The number comes from (`seed`, the note's index in the take).
pub fn random_velocity(
    doc: &Doc,
    clip: ClipId,
    indexes: &[usize],
    amount: u8,
    seed: u32,
) -> Result<NotesEdited, EditError> {
    let amount = amount.clamp(*RANDOM_AMOUNT.start(), *RANDOM_AMOUNT.end());
    change_picked(
        doc,
        clip,
        indexes,
        |count| StepKind::RandomVelocity { clip, count },
        |picked| {
            for (i, n) in picked {
                let delta = random_delta(seed, *i, amount);
                n.velocity = velocity(i64::from(n.velocity) + delta);
            }
        },
    )
}

/// The number [`random_velocity`] adds to the note at `index`.
pub fn random_delta(seed: u32, index: usize, amount: u8) -> i64 {
    let n = u64::from(amount);
    let r = splitmix64((u64::from(seed) << 32) ^ index as u64);
    (r % (2 * n + 1)) as i64 - n as i64
}

/// Each picked note ends at the nearest later start among the picked notes (shorter too);
/// the notes of the latest start stay as they are.
pub fn legato(doc: &Doc, clip: ClipId, indexes: &[usize]) -> Result<NotesEdited, EditError> {
    change_picked(
        doc,
        clip,
        indexes,
        |count| StepKind::Legato { clip, count },
        |picked| {
            let mut starts: Vec<u64> = picked.iter().map(|(_, n)| n.start).collect();
            starts.sort_unstable();
            starts.dedup();
            for (_, n) in picked {
                let next = starts.partition_point(|&s| s <= n.start);
                if let Some(&s) = starts.get(next) {
                    n.length = s - n.start;
                }
            }
        },
    )
}

/// Like [`change_each`], with the picked notes (their index in the take, and the note) all
/// given to `change` at once.
fn change_picked(
    doc: &Doc,
    clip: ClipId,
    indexes: &[usize],
    step: impl FnOnce(usize) -> StepKind,
    change: impl FnOnce(&mut [(usize, Note)]),
) -> Result<NotesEdited, EditError> {
    let (ti, ci) = keys_clip_index(doc, clip)?;
    let notes = song_notes(&lifted(&doc.tracks[ti].clips[ci]).0);
    let flags = pick(indexes, notes.len())?;
    let count = indexes_count(&flags);
    let mut picked: Vec<(usize, Note)> = notes
        .iter()
        .enumerate()
        .filter(|&(i, _)| flags[i])
        .map(|(i, n)| (i, *n))
        .collect();
    change(&mut picked);
    let mut changed = picked.into_iter().map(|(_, n)| n);
    let out = notes
        .into_iter()
        .zip(flags)
        .map(|(n, p)| {
            if p {
                (changed.next().expect("one per pick"), true)
            } else {
                (n, false)
            }
        })
        .collect();
    put_notes(doc, (ti, ci), out, step(count), Reach::NONE)
}

#[cfg(test)]
mod tests;
