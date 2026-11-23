//! Surge XT's patch files (`.fxp`): a 60-byte header in big endian (`CcnK`, a size Surge
//! leaves 0, `FPCh`, version 1, the id `cjs3`, version 1, one program, the name in 28
//! bytes, the size of the chunk), then the patch chunk ([`super::chunk`]).

use super::chunk;

const HEADER: usize = 60;
const NAME: usize = 28;

fn be32(bytes: &[u8], at: usize) -> u32 {
    u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]])
}

/// The patch chunk of a `.fxp` file's bytes. Bytes after the chunk are passed over, as Surge
/// XT does (one factory patch has one).
pub fn chunk_of(file: &[u8]) -> Result<&[u8], String> {
    let bad = |what: &str| format!("not a Surge XT patch file ({what})");
    if file.len() < HEADER {
        return Err(bad("too short"));
    }
    if &file[0..4] != b"CcnK" || &file[8..12] != b"FPCh" || &file[16..20] != b"cjs3" {
        return Err(bad("its header is not Surge XT's"));
    }
    let len = be32(file, 56) as usize;
    let Some(bytes) = file.get(HEADER..HEADER + len) else {
        return Err(bad("cut short"));
    };
    let parsed = chunk::parse(bytes)?;
    if parsed.len != len {
        return Err(bad("its chunk is not as long as its header says"));
    }
    Ok(bytes)
}

/// A `.fxp` file of the chunk `bytes` (checked), named `name` (cut to 27 bytes).
pub fn file_of(bytes: &[u8], name: &str) -> Result<Vec<u8>, String> {
    let parsed = chunk::parse(bytes)?;
    if parsed.len != bytes.len() {
        return Err("the patch has bytes after it".into());
    }
    let mut out = Vec::with_capacity(HEADER + bytes.len());
    out.extend_from_slice(b"CcnK");
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(b"FPCh");
    out.extend_from_slice(&1u32.to_be_bytes());
    out.extend_from_slice(b"cjs3");
    out.extend_from_slice(&1u32.to_be_bytes());
    out.extend_from_slice(&1u32.to_be_bytes());
    let mut field = [0u8; NAME];
    let mut cut = name.len().min(NAME - 1);
    while !name.is_char_boundary(cut) {
        cut -= 1;
    }
    field[..cut].copy_from_slice(&name.as_bytes()[..cut]);
    out.extend_from_slice(&field);
    out.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    out.extend_from_slice(bytes);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::super::chunk::tests::made;
    use super::*;

    #[test]
    fn a_file_holds_the_chunk() {
        let bytes = made("<patch revision=\"24\"/>", true);
        let file = file_of(&bytes, "A name longer than twenty-seven bytes").unwrap();
        assert_eq!(file.len(), 60 + bytes.len());
        assert_eq!(&file[28..56], b"A name longer than twenty-s\0");
        assert_eq!(chunk_of(&file).unwrap(), &bytes[..]);
        let mut longer = file.clone();
        longer.push(0);
        assert_eq!(chunk_of(&longer).unwrap(), &bytes[..]);
        assert!(file_of(&longer[60..], "x").is_err());
        let jp = file_of(&bytes, "あいうえおかきくけこ").unwrap();
        assert_eq!(&jp[28..55], "あいうえおかきくけ".as_bytes());
    }

    #[test]
    fn it_refuses_other_files() {
        let file = file_of(&made("<patch/>", false), "x").unwrap();
        let mut id = file.clone();
        id[16..20].copy_from_slice(b"sub3");
        let mut long = file.clone();
        long[56..60].copy_from_slice(&1000u32.to_be_bytes());
        let mut short = file.clone();
        short[56..60].copy_from_slice(&39u32.to_be_bytes());
        for (what, bad) in [
            ("id", &id[..]),
            ("long", &long[..]),
            ("short", &short[..]),
            ("cut", &file[..59]),
            ("chunk alone", &file[60..]),
        ] {
            assert!(chunk_of(bad).is_err(), "{what}");
        }
    }
}
