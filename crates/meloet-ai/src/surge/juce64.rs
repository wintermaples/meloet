//! The base64 of JUCE's `MemoryBlock`, which JUCE's LV2 plugins keep their state in: the
//! byte count in decimal, a dot, then 6 bits a character taken from the lowest bit of the
//! first byte up, in an alphabet of its own, with no padding.

const ALPHABET: &[u8; 64] = b".ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+";

fn chars_for(len: usize) -> usize {
    (len * 8).div_ceil(6)
}

pub fn encode(bytes: &[u8]) -> String {
    let mut out = format!("{}.", bytes.len());
    out.reserve(chars_for(bytes.len()));
    let bit = |at: usize| bytes.get(at / 8).is_some_and(|b| b >> (at % 8) & 1 == 1);
    for i in 0..chars_for(bytes.len()) {
        let six = (0..6).fold(0, |six, b| six | usize::from(bit(i * 6 + b)) << b);
        out.push(char::from(ALPHABET[six]));
    }
    out
}

fn six(c: u8) -> Option<u8> {
    ALPHABET.iter().position(|a| *a == c).map(|i| i as u8)
}

/// `None` unless the text is exactly what [`encode`] writes for some bytes, at most `most`
/// of them.
pub fn decode(text: &str, most: usize) -> Option<Vec<u8>> {
    let (count, chars) = text.split_once('.')?;
    if count.is_empty() || count.len() > 10 || !count.bytes().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let len: usize = count.parse().ok()?;
    if len > most || chars.len() != chars_for(len) {
        return None;
    }
    let mut out = vec![0u8; len];
    for (i, c) in chars.bytes().enumerate() {
        let six = six(c)?;
        for b in 0..6 {
            let at = i * 6 + b;
            if six >> b & 1 == 1 {
                // Bits past the last byte must be 0, as `encode` writes them.
                *out.get_mut(at / 8)? |= 1 << (at % 8);
            }
        }
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_goes_both_ways() {
        assert_eq!(encode(&[0x01]), "1.A.");
        assert_eq!(decode("1.A.", 10), Some(vec![0x01]));
        assert_eq!(encode(&[]), "0.");
        assert_eq!(decode("0.", 10), Some(vec![]));
        let all: Vec<u8> = (0..=255).collect();
        for n in 0..all.len() {
            assert_eq!(decode(&encode(&all[..n]), 1000).as_deref(), Some(&all[..n]));
        }
        // "sub3": what the start of every state of Surge XT is.
        assert_eq!(encode(b"sub3"), "4.yUmXy.");
    }

    #[test]
    fn it_refuses_anything_else() {
        for bad in [
            "", "1", "1.", "1.A", "1.A..", "x.A.", "1.A/", "2.A.", "-1.A.", " 1.A.", "1.AE",
        ] {
            assert_eq!(decode(bad, 10), None, "{bad:?}");
        }
        assert_eq!(decode("4.yUmXyX", 3), None);
    }
}
