//! Surge XT's patch inside meloet's plugin states (the plugin host's containers, see
//! `tools/plugstate/readable.rs`).
//!
//! - VST3: the component's state is the patch chunk, then JUCE's trailer: 8 zero bytes, the
//!   bytes of a value tree, their count (u64, little endian), `JUCEPrivateData`. Surge XT
//!   writes no value tree. The controller's state is left as it is.
//! - LV2: lilv's Turtle holds the chunk in the property `…/surge-xt:StateString` as JUCE's
//!   base64 ([`super::juce64`]). It must hold no `…/surge-xt:Program`: JUCE takes a program
//!   before the string.

use std::borrow::Cow;
use std::ops::Range;

use super::{chunk, juce64};

const VST3: &[u8; 4] = b"MV3S";
const LV2: &[u8; 4] = b"ML2S";
const JUCE: &[u8; 15] = b"JUCEPrivateData";
const STATE_STRING: &str = "<https://surge-synthesizer.github.io/lv2/surge-xt:StateString>";
const PROGRAM: &str = "surge-xt:Program";
/// The most a state may be (as `tools/plugstate.rs` takes).
const MOST: usize = 64 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Vst3,
    Lv2,
}

impl Format {
    /// From a plugin key (`vst3:…`, `lv2:…`).
    pub fn of_key(key: &str) -> Option<Format> {
        match key.split_once(':')?.0 {
            "vst3" => Some(Format::Vst3),
            "lv2" => Some(Format::Lv2),
            _ => None,
        }
    }
}

fn unknown() -> String {
    "the plugin's state is not in a form meloet knows for Surge XT".into()
}

/// Where the patch is in a VST3 state.
struct Vst3<'a> {
    component: Range<usize>,
    chunk_len: usize,
    state: &'a [u8],
}

fn vst3(state: &[u8]) -> Result<Vst3<'_>, String> {
    if state.len() < 12 || &state[..4] != VST3 {
        return Err(unknown());
    }
    let len = u64::from_le_bytes(state[4..12].try_into().unwrap_or_default());
    let component = usize::try_from(len)
        .ok()
        .and_then(|len| len.checked_add(12))
        .filter(|end| *end <= state.len())
        .map(|end| 12..end)
        .ok_or_else(unknown)?;
    let bytes = &state[component.clone()];
    let chunk_len = chunk::parse(bytes)?.len;
    let trailer = &bytes[chunk_len..];
    if !trailer.is_empty() {
        let n = trailer.len();
        if n < 31 || !trailer.ends_with(JUCE) || trailer[..8] != [0; 8] {
            return Err(unknown());
        }
        let tree = u64::from_le_bytes(trailer[n - 23..n - 15].try_into().unwrap_or_default());
        if usize::try_from(tree).ok().and_then(|t| t.checked_add(31)) != Some(n) {
            return Err(unknown());
        }
    }
    Ok(Vst3 {
        component,
        chunk_len,
        state,
    })
}

/// Where the patch is in an LV2 state: the text of the string, between its quotes.
struct Lv2<'a> {
    turtle_at: usize,
    literal: Range<usize>,
    state: &'a [u8],
}

fn lv2(state: &[u8]) -> Result<Lv2<'_>, String> {
    if state.len() < 8 || &state[..4] != LV2 {
        return Err(unknown());
    }
    let count = u32::from_le_bytes(state[4..8].try_into().unwrap_or_default()) as usize;
    let turtle_at = count
        .checked_mul(12)
        .and_then(|n| n.checked_add(8))
        .filter(|at| *at <= state.len())
        .ok_or_else(unknown)?;
    let turtle = std::str::from_utf8(&state[turtle_at..]).map_err(|_| unknown())?;
    if turtle.contains(PROGRAM) {
        return Err(
            "the state names a program of Surge XT, which would be taken in place of \
                    its patch"
                .into(),
        );
    }
    let mut found = turtle.match_indices(STATE_STRING);
    let (Some((at, _)), None) = (found.next(), found.next()) else {
        return Err(unknown());
    };
    let after = at + STATE_STRING.len();
    let rest = &turtle[after..];
    let quote = after + (rest.len() - rest.trim_start().len());
    if turtle.as_bytes().get(quote) != Some(&b'"') {
        return Err(unknown());
    }
    let len = turtle[quote + 1..].find('"').ok_or_else(unknown)?;
    Ok(Lv2 {
        turtle_at,
        literal: turtle_at + quote + 1..turtle_at + quote + 1 + len,
        state,
    })
}

/// The patch chunk inside a state of the plugin.
pub fn chunk_of(format: Format, state: &[u8]) -> Result<Cow<'_, [u8]>, String> {
    match format {
        Format::Vst3 => {
            let found = vst3(state)?;
            let at = found.component.start;
            Ok(Cow::Borrowed(&state[at..at + found.chunk_len]))
        }
        Format::Lv2 => {
            let found = lv2(state)?;
            let text = std::str::from_utf8(&state[found.literal]).map_err(|_| unknown())?;
            let bytes = juce64::decode(text, MOST).ok_or_else(unknown)?;
            let len = chunk::parse(&bytes)?.len;
            if len != bytes.len() {
                return Err(unknown());
            }
            Ok(Cow::Owned(bytes))
        }
    }
}

/// The state with the patch chunk `chunk` (checked) in place of its own; everything else as
/// it was, but for the LV2 container's parameter values, which were of the patch before.
pub fn with_chunk(format: Format, state: &[u8], new: &[u8]) -> Result<Vec<u8>, String> {
    if chunk::parse(new)?.len != new.len() {
        return Err("the patch has bytes after it".into());
    }
    chunk_of(format, state)?;
    match format {
        Format::Vst3 => {
            let found = vst3(state)?;
            let component = &found.state[found.component.clone()];
            let trailer = &component[found.chunk_len..];
            let len = (new.len() + trailer.len()) as u64;
            let mut out = Vec::with_capacity(state.len() + new.len());
            out.extend_from_slice(VST3);
            out.extend_from_slice(&len.to_le_bytes());
            out.extend_from_slice(new);
            out.extend_from_slice(trailer);
            out.extend_from_slice(&found.state[found.component.end..]);
            Ok(out)
        }
        Format::Lv2 => {
            let found = lv2(state)?;
            let text = juce64::encode(new);
            let mut out = Vec::with_capacity(state.len() + text.len());
            out.extend_from_slice(LV2);
            out.extend_from_slice(&0u32.to_le_bytes());
            out.extend_from_slice(&found.state[found.turtle_at..found.literal.start]);
            out.extend_from_slice(text.as_bytes());
            out.extend_from_slice(&found.state[found.literal.end..]);
            Ok(out)
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::super::chunk::tests::made;
    use super::*;

    pub fn vst3_state(chunk: &[u8]) -> Vec<u8> {
        let mut component = chunk.to_vec();
        component.extend_from_slice(&[0; 16]);
        component.extend_from_slice(JUCE);
        let mut out = VST3.to_vec();
        out.extend_from_slice(&(component.len() as u64).to_le_bytes());
        out.extend_from_slice(&component);
        out.extend_from_slice(b"ctl");
        out
    }

    /// As lilv writes Surge XT's state (the shape of a real one, made smaller).
    pub fn lv2_state(chunk: &[u8], pairs: u32) -> Vec<u8> {
        let mut out = LV2.to_vec();
        out.extend_from_slice(&pairs.to_le_bytes());
        for i in 0..pairs {
            out.extend_from_slice(&i.to_le_bytes());
            out.extend_from_slice(&0.5f64.to_le_bytes());
        }
        let turtle = format!(
            "<urn:meloet:state>\n\ta pset:Preset ;\n\tlv2:appliesTo \
             <https://surge-synthesizer.github.io/lv2/surge-xt> ;\n\tlv2:port [\n\t\t\
             lv2:symbol \"enabled\" ;\n\t\tpset:value 1.0\n\t] ;\n\tstate:state [\n\t\t\
             {STATE_STRING} \"{}\"\n\t] .\n",
            juce64::encode(chunk)
        );
        out.extend_from_slice(turtle.as_bytes());
        out
    }

    #[test]
    fn the_patch_is_found_and_replaced_in_a_vst3_state() {
        let old = made("<patch revision=\"24\"/>", true);
        let new = made("<patch revision=\"24\"><x/></patch>", true);
        let state = vst3_state(&old);
        assert_eq!(&*chunk_of(Format::Vst3, &state).unwrap(), &old[..]);
        let put = with_chunk(Format::Vst3, &state, &new).unwrap();
        assert_eq!(put, vst3_state(&new));
        assert_eq!(&*chunk_of(Format::Vst3, &put).unwrap(), &new[..]);
        // No trailer at all is taken too.
        let mut bare = VST3.to_vec();
        bare.extend_from_slice(&(old.len() as u64).to_le_bytes());
        bare.extend_from_slice(&old);
        assert_eq!(&*chunk_of(Format::Vst3, &bare).unwrap(), &old[..]);
    }

    #[test]
    fn the_patch_is_found_and_replaced_in_an_lv2_state() {
        let old = made("<patch revision=\"24\"/>", false);
        let new = made("<patch revision=\"24\"><x/></patch>", true);
        let state = lv2_state(&old, 2);
        assert_eq!(&*chunk_of(Format::Lv2, &state).unwrap(), &old[..]);
        let put = with_chunk(Format::Lv2, &state, &new).unwrap();
        assert_eq!(put, lv2_state(&new, 0));
        assert_eq!(&*chunk_of(Format::Lv2, &put).unwrap(), &new[..]);
    }

    #[test]
    fn broken_states_are_refused() {
        let chunk = made("<patch/>", false);
        let vst3 = vst3_state(&chunk);
        let lv2 = lv2_state(&chunk, 0);
        let mut long = vst3.clone();
        long[4] = 0xff;
        let mut trailer = vst3.clone();
        let n = 12 + chunk.len();
        trailer[n] = 1;
        let mut tree = vst3.clone();
        tree[n + 8] = 5;
        let half = &vst3[..12 + chunk.len() / 2];
        let program = String::from_utf8(lv2.clone()).unwrap().replace(
            "\t] .",
            "\t\t<https://surge-synthesizer.github.io/lv2/surge-xt:Program> 0 ;\n\t] .",
        );
        let twice = String::from_utf8(lv2.clone())
            .unwrap()
            .replace("\t] .", &format!("\t\t{STATE_STRING} \"0.\"\n\t] ."));
        let garbage = String::from_utf8(lv2.clone())
            .unwrap()
            .replace("\"40.", "\"41.");
        let mut pairs = lv2.clone();
        pairs[4] = 200;
        for (what, format, bad) in [
            ("vst3 long", Format::Vst3, &long[..]),
            ("vst3 trailer", Format::Vst3, &trailer[..]),
            ("vst3 tree", Format::Vst3, &tree[..]),
            ("vst3 half", Format::Vst3, half),
            ("vst3 as lv2", Format::Lv2, &vst3[..]),
            ("lv2 as vst3", Format::Vst3, &lv2[..]),
            ("lv2 program", Format::Lv2, program.as_bytes()),
            ("lv2 twice", Format::Lv2, twice.as_bytes()),
            ("lv2 garbage", Format::Lv2, garbage.as_bytes()),
            ("lv2 pairs", Format::Lv2, &pairs[..]),
        ] {
            assert!(chunk_of(format, bad).is_err(), "{what}");
            assert!(with_chunk(format, bad, &chunk).is_err(), "{what}");
        }
        let mut more = chunk.clone();
        more.push(0);
        assert!(with_chunk(Format::Vst3, &vst3, &more).is_err());
    }
}
