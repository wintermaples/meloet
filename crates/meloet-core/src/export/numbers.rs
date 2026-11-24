//! The track numbers of 「番号で」 of a MIDI export, as 「1-3, 5」 (spec 7 A-31).
//!
//! Full-width digits are digits; 「、」 and 「，」 are commas; 「－」 「ー」 「‐」 「〜」 「～」 「−」 「~」 are
//! hyphens. Spaces around commas and hyphens do not count, a space between two digits
//! cannot be read. Empty parts are passed by, a range written backwards is read forwards,
//! a number named twice is taken once.

/// What the text names of tracks numbered 1 to `count`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Numbers {
    Read {
        /// The numbers that are tracks, rising, each once.
        tracks: Vec<u32>,
        /// The smallest number named that is no track's (0, or past `count`).
        missing: Option<u32>,
    },
    Unreadable,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Sign {
    Digit(u8),
    Comma,
    Hyphen,
    Space,
}

fn sign(c: char) -> Option<Sign> {
    Some(match c {
        '0'..='9' => Sign::Digit(c as u8 - b'0'),
        '０'..='９' => Sign::Digit((c as u32 - '０' as u32) as u8),
        ',' | '、' | '，' => Sign::Comma,
        '-' | '－' | 'ー' | '‐' | '〜' | '～' | '−' | '~' => Sign::Hyphen,
        c if c.is_whitespace() => Sign::Space,
        _ => return None,
    })
}

/// A number of one part, or `None` when it cannot be read: no digits, a space between
/// digits, or more than a `u32` holds.
fn number(signs: &[Sign]) -> Option<u32> {
    let mut digits = signs.iter().skip_while(|s| **s == Sign::Space).peekable();
    let mut value: u32 = 0;
    let mut any = false;
    while let Some(Sign::Digit(d)) = digits.peek() {
        value = value.checked_mul(10)?.checked_add(u32::from(*d))?;
        any = true;
        digits.next();
    }
    (any && digits.all(|s| *s == Sign::Space)).then_some(value)
}

pub(crate) fn read(text: &str, count: u32) -> Numbers {
    let Some(signs) = text.chars().map(sign).collect::<Option<Vec<Sign>>>() else {
        return Numbers::Unreadable;
    };
    let mut ranges = Vec::new();
    for part in signs.split(|s| *s == Sign::Comma) {
        if part.iter().all(|s| *s == Sign::Space) {
            continue;
        }
        let mut ends = part.split(|s| *s == Sign::Hyphen);
        let (Some(first), second, None) = (ends.next(), ends.next(), ends.next()) else {
            return Numbers::Unreadable;
        };
        let Some(a) = number(first) else {
            return Numbers::Unreadable;
        };
        let b = match second {
            Some(second) => match number(second) {
                Some(b) => b,
                None => return Numbers::Unreadable,
            },
            None => a,
        };
        ranges.push((a.min(b), a.max(b)));
    }
    let mut tracks = Vec::new();
    let mut missing: Option<u32> = None;
    for &(low, high) in &ranges {
        let gap = match low {
            0 => Some(0),
            _ if high > count => Some(low.max(count + 1)),
            _ => None,
        };
        if let Some(gap) = gap {
            missing = Some(missing.map_or(gap, |m| m.min(gap)));
        }
        if low <= count && high >= 1 {
            tracks.extend(low.max(1)..=high.min(count));
        }
    }
    tracks.sort_unstable();
    tracks.dedup();
    Numbers::Read { tracks, missing }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracks(text: &str, count: u32) -> (Vec<u32>, Option<u32>) {
        match read(text, count) {
            Numbers::Read { tracks, missing } => (tracks, missing),
            Numbers::Unreadable => panic!("{text:?} was not read"),
        }
    }

    #[test]
    fn numbers_and_ranges_split_by_commas() {
        assert_eq!(tracks("1-3, 5", 6), (vec![1, 2, 3, 5], None));
        assert_eq!(tracks("2", 6), (vec![2], None));
        assert_eq!(tracks("1,2,3", 3), (vec![1, 2, 3], None));
        assert_eq!(tracks("4-4", 6), (vec![4], None));
        assert_eq!(tracks("007", 9), (vec![7], None));
    }

    #[test]
    fn spaces_around_commas_and_hyphens_do_not_count() {
        assert_eq!(tracks("  1 - 3 ,5  ", 6), (vec![1, 2, 3, 5], None));
        assert_eq!(tracks("1 -3", 6), (vec![1, 2, 3], None));
        assert_eq!(tracks("\t2\t", 6), (vec![2], None));
        // An ideographic space is a space.
        assert_eq!(tracks("1\u{3000}-\u{3000}2", 6), (vec![1, 2], None));
    }

    #[test]
    fn a_space_between_two_digits_cannot_be_read() {
        assert_eq!(read("1 2", 6), Numbers::Unreadable);
        assert_eq!(read("1, 2 3", 6), Numbers::Unreadable);
        assert_eq!(read("1-2 3", 6), Numbers::Unreadable);
    }

    #[test]
    fn what_the_japanese_input_types_is_read() {
        assert_eq!(tracks("１－３、５", 6), (vec![1, 2, 3, 5], None));
        assert_eq!(tracks("１ー２，４", 6), (vec![1, 2, 4], None));
        assert_eq!(tracks("2‐3", 6), (vec![2, 3], None));
        assert_eq!(tracks("2〜3", 6), (vec![2, 3], None));
        assert_eq!(tracks("2～3", 6), (vec![2, 3], None));
        assert_eq!(tracks("2−3", 6), (vec![2, 3], None));
        assert_eq!(tracks("2~3", 6), (vec![2, 3], None));
        assert_eq!(tracks("１２", 12), (vec![12], None));
        // Full-width and plain digits in one number.
        assert_eq!(tracks("1２", 12), (vec![12], None));
    }

    #[test]
    fn empty_parts_are_passed_by() {
        assert_eq!(tracks("1,,2,", 6), (vec![1, 2], None));
        assert_eq!(tracks(",3", 6), (vec![3], None));
        assert_eq!(tracks(" , ", 6), (vec![], None));
        assert_eq!(tracks("", 6), (vec![], None));
        assert_eq!(tracks("   ", 6), (vec![], None));
    }

    #[test]
    fn a_range_written_backwards_is_read_forwards_and_a_number_is_taken_once() {
        assert_eq!(tracks("5-3", 6), (vec![3, 4, 5], None));
        assert_eq!(tracks("3, 1-4, 3, 2", 6), (vec![1, 2, 3, 4], None));
        // In the order of the tracks, whatever the order of the text.
        assert_eq!(tracks("6, 2, 4", 6), (vec![2, 4, 6], None));
    }

    #[test]
    fn the_smallest_number_with_no_track_is_said_and_the_others_are_still_read() {
        assert_eq!(tracks("1-3, 8", 4), (vec![1, 2, 3], Some(8)));
        assert_eq!(tracks("9, 2, 7", 4), (vec![2], Some(7)));
        // A range past the end: its first number past the end.
        assert_eq!(tracks("3-9", 4), (vec![3, 4], Some(5)));
        assert_eq!(tracks("9-3", 4), (vec![3, 4], Some(5)));
        assert_eq!(tracks("6-9", 4), (vec![], Some(6)));
        // 0 is a number with no track.
        assert_eq!(tracks("0", 4), (vec![], Some(0)));
        assert_eq!(tracks("2, 0-1", 4), (vec![1, 2], Some(0)));
        assert_eq!(tracks("1", 0), (vec![], Some(1)));
        // A range of billions is not counted out.
        assert_eq!(tracks("1-4000000000", 3), (vec![1, 2, 3], Some(4)));
        assert_eq!(tracks("4294967295", 3), (vec![], Some(4_294_967_295)));
    }

    #[test]
    fn what_is_not_numbers_cannot_be_read() {
        for text in [
            "a",
            "1-",
            "-3",
            "-",
            "1--2",
            "1-2-3",
            "1.5",
            "1;2",
            "+1",
            "一",
            "１a",
            // Too large for a number.
            "4294967296",
            "99999999999999999999",
            "1-4294967296",
        ] {
            assert_eq!(read(text, 6), Numbers::Unreadable, "{text:?}");
        }
    }
}
