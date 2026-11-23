//! Surge XT's patch as bytes (what its `.fxp` files hold after their header, and what the
//! plugin says as its state): a 32-byte header in little endian (`sub3`, the size of the
//! XML, then the size of an embedded wavetable for each of 2 scenes × 3 oscillators), the
//! patch XML, then those wavetables in that order.

pub const TAG: &[u8; 4] = b"sub3";
const HEADER: usize = 32;
/// The most XML Surge XT reads.
pub const MAX_XML: usize = 4 << 20;
const WAVETABLES: usize = 6;
/// A wavetable block's own header: 4 bytes Surge leaves as they are, the size of a wave
/// (u32), the number of waves (u16) and flags (u16).
const WT_HEADER: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chunk<'a> {
    /// UTF-8 in every patch Surge XT writes now; a few old ones are not.
    pub xml: &'a [u8],
    /// The wavetable blocks, as they follow the XML.
    pub wavetables: &'a [u8],
    pub wt_sizes: [u32; WAVETABLES],
    /// How long the whole chunk is.
    pub len: usize,
}

impl<'a> Chunk<'a> {
    pub fn text(&self) -> Result<&'a str, String> {
        std::str::from_utf8(self.xml).map_err(|_| {
            "the patch's XML is not UTF-8 text (an old patch); meloet does not edit it".into()
        })
    }
}

fn le32(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

/// The chunk at the start of `bytes`; what follows it is not looked at.
pub fn parse(bytes: &[u8]) -> Result<Chunk<'_>, String> {
    if bytes.len() < HEADER || &bytes[..4] != TAG {
        return Err("not a Surge XT patch (no \"sub3\" header)".into());
    }
    let xml_len = le32(bytes, 4) as usize;
    if xml_len > MAX_XML {
        return Err(format!(
            "the patch XML is {xml_len} bytes, more than Surge XT reads"
        ));
    }
    let wt_sizes: [u32; WAVETABLES] = std::array::from_fn(|i| le32(bytes, 8 + 4 * i));
    let wt_len = wt_sizes.iter().map(|s| *s as usize).sum::<usize>();
    let len = HEADER + xml_len + wt_len;
    if bytes.len() < len {
        return Err(format!(
            "the patch is cut short: {} bytes where its header says {len}",
            bytes.len()
        ));
    }
    let xml = &bytes[HEADER..HEADER + xml_len];
    let wavetables = &bytes[HEADER + xml_len..len];
    let mut at = 0;
    for size in wt_sizes.iter().map(|s| *s as usize).filter(|s| *s > 0) {
        check_wavetable(&wavetables[at..at + size])?;
        at += size;
    }
    Ok(Chunk {
        xml,
        wavetables,
        wt_sizes,
        len,
    })
}

/// A wavetable block is its header and exactly the waves it says.
fn check_wavetable(block: &[u8]) -> Result<(), String> {
    let bad = || "an embedded wavetable of the patch is broken".to_owned();
    if block.len() < WT_HEADER {
        return Err(bad());
    }
    let wave = le32(block, 4) as usize;
    let count = usize::from(u16::from_le_bytes([block[8], block[9]]));
    let flags = u16::from_le_bytes([block[10], block[11]]);
    let sample = if flags & 4 != 0 { 2 } else { 4 };
    // A sample (flag 1) may have more than the 512 waves of a wavetable: one factory patch
    // has 2051.
    if !wave.is_power_of_two() || !(2..=4096).contains(&wave) || count == 0 {
        return Err(bad());
    }
    if WT_HEADER + wave * count * sample != block.len() {
        return Err(bad());
    }
    Ok(())
}

/// The chunk with `xml` in place of its XML, its wavetables as they were.
pub fn with_xml(chunk: &Chunk, xml: &str) -> Result<Vec<u8>, String> {
    if xml.len() > MAX_XML {
        return Err(format!(
            "the patch XML is {} bytes, more than Surge XT reads ({MAX_XML})",
            xml.len()
        ));
    }
    let mut out = Vec::with_capacity(HEADER + xml.len() + chunk.wavetables.len());
    out.extend_from_slice(TAG);
    out.extend_from_slice(&(xml.len() as u32).to_le_bytes());
    for size in chunk.wt_sizes {
        out.extend_from_slice(&size.to_le_bytes());
    }
    out.extend_from_slice(xml.as_bytes());
    out.extend_from_slice(chunk.wavetables);
    Ok(out)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A chunk of our own making: `xml` and one small wavetable for scene A's second
    /// oscillator when `wavetable`.
    pub fn made(xml: &str, wavetable: bool) -> Vec<u8> {
        let mut block = Vec::new();
        if wavetable {
            block.extend_from_slice(&[0; 4]);
            block.extend_from_slice(&4u32.to_le_bytes());
            block.extend_from_slice(&2u16.to_le_bytes());
            block.extend_from_slice(&4u16.to_le_bytes());
            block.extend((0..16).map(|i| i as u8));
        }
        let mut out = TAG.to_vec();
        out.extend_from_slice(&(xml.len() as u32).to_le_bytes());
        for i in 0..6 {
            let size = if i == 1 { block.len() as u32 } else { 0 };
            out.extend_from_slice(&size.to_le_bytes());
        }
        out.extend_from_slice(xml.as_bytes());
        out.extend_from_slice(&block);
        out
    }

    #[test]
    fn it_reads_and_writes_the_xml_and_keeps_the_wavetables() {
        let bytes = made("<patch revision=\"24\"/>", true);
        let mut longer = bytes.clone();
        longer.extend_from_slice(b"after");
        let chunk = parse(&longer).unwrap();
        assert_eq!(chunk.text(), Ok("<patch revision=\"24\"/>"));
        assert_eq!(chunk.len, bytes.len());
        assert_eq!(chunk.wavetables.len(), 28);
        let new = with_xml(&chunk, "<patch revision=\"24\"><x/></patch>").unwrap();
        assert_eq!(new, made("<patch revision=\"24\"><x/></patch>", true));
        assert_eq!(with_xml(&chunk, chunk.text().unwrap()).unwrap(), bytes);
        let mut latin1 = made("<patch name=\"x\"/>", false);
        latin1[45] = 0xb5;
        let old = parse(&latin1).unwrap();
        assert!(old.text().is_err());
        assert_eq!(old.len, latin1.len());
    }

    #[test]
    fn it_refuses_broken_chunks() {
        let good = made("<patch/>", true);
        assert!(parse(&good).is_ok());
        let mut cut = good.clone();
        cut.pop();
        let mut tag = good.clone();
        tag[0] = b'S';
        let mut huge = good.clone();
        huge[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        let mut waves = good.clone();
        let wt = 32 + 8;
        waves[wt + 8] = 3;
        for (what, bad) in [
            ("cut", &cut[..]),
            ("tag", &tag[..]),
            ("huge", &huge[..]),
            ("waves", &waves[..]),
            ("short", &good[..20]),
        ] {
            assert!(parse(bad).is_err(), "{what}");
        }
    }
}
