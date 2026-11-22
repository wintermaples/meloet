//! Unpacks the tar files of the sound pack: plain ustar with files and folders only, as
//! `tools/sounds/split.py` writes them. Anything else (links, devices, pax headers, names that
//! leave the folder) refuses the whole file.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Component, Path, PathBuf};

const BLOCK: usize = 512;

fn bad(what: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what.into())
}

fn field(header: &[u8], at: usize, len: usize) -> &[u8] {
    let f = &header[at..at + len];
    let end = f.iter().position(|&b| b == 0).unwrap_or(len);
    &f[..end]
}

fn octal(header: &[u8], at: usize, len: usize) -> io::Result<u64> {
    let text = std::str::from_utf8(field(header, at, len)).map_err(|_| bad("number"))?;
    let text = text.trim_matches(' ');
    if text.is_empty() {
        return Ok(0);
    }
    u64::from_str_radix(text, 8).map_err(|_| bad(format!("number {text:?}")))
}

/// The name of an entry as a path inside the folder: no root, no `..`, nothing odd.
fn inside(header: &[u8]) -> io::Result<PathBuf> {
    let name = field(header, 0, 100);
    let prefix = field(header, 345, 155);
    let mut full = Vec::with_capacity(prefix.len() + 1 + name.len());
    if !prefix.is_empty() {
        full.extend_from_slice(prefix);
        full.push(b'/');
    }
    full.extend_from_slice(name);
    let text = std::str::from_utf8(&full).map_err(|_| bad("name"))?;
    let text = text.strip_suffix('/').unwrap_or(text);
    let path = Path::new(text);
    let fine = !text.is_empty()
        && !text.contains('\\')
        && path.components().all(|c| matches!(c, Component::Normal(_)));
    if !fine {
        return Err(bad(format!("name {text:?}")));
    }
    Ok(path.to_path_buf())
}

/// Unpacks `tar` into the folder `into`, which must be empty or not there yet.
pub fn unpack(tar: &Path, into: &Path) -> io::Result<()> {
    fs::create_dir_all(into)?;
    let mut file = io::BufReader::with_capacity(1 << 16, File::open(tar)?);
    let mut header = [0u8; BLOCK];
    let mut buffer = vec![0u8; 1 << 16];
    loop {
        file.read_exact(&mut header)?;
        if header.iter().all(|&b| b == 0) {
            return Ok(());
        }
        let sum = octal(&header, 148, 8)?;
        let counted: u64 = (header.iter().enumerate())
            .map(|(i, &b)| u64::from(if (148..156).contains(&i) { b' ' } else { b }))
            .sum();
        if sum != counted {
            return Err(bad("header checksum"));
        }
        if &header[257..263] != b"ustar\0" {
            return Err(bad("not ustar"));
        }
        let path = into.join(inside(&header)?);
        let size = octal(&header, 124, 12)?;
        match header[156] {
            b'5' if size == 0 => fs::create_dir_all(&path)?,
            b'0' | 0 => {
                if let Some(parent) = path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut out = File::create_new(&path)?;
                let mut left = size;
                while left > 0 {
                    let n = left.min(buffer.len() as u64) as usize;
                    file.read_exact(&mut buffer[..n])?;
                    out.write_all(&buffer[..n])?;
                    left -= n as u64;
                }
                out.sync_all()?;
                let pad = (BLOCK as u64 - size % BLOCK as u64) % BLOCK as u64;
                file.read_exact(&mut buffer[..pad as usize])?;
            }
            other => return Err(bad(format!("entry of type {:?}", other as char))),
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// A ustar file with these entries (`None`: a folder), as split.py writes one.
    pub(crate) fn tar_of(entries: &[(&str, Option<&[u8]>)]) -> Vec<u8> {
        let mut out = Vec::new();
        for (name, data) in entries {
            let mut h = [0u8; BLOCK];
            h[..name.len()].copy_from_slice(name.as_bytes());
            let size = data.map_or(0, <[u8]>::len);
            h[100..107].copy_from_slice(b"0000644");
            h[108..115].copy_from_slice(b"0000000");
            h[116..123].copy_from_slice(b"0000000");
            h[124..135].copy_from_slice(format!("{size:011o}").as_bytes());
            h[136..147].copy_from_slice(b"00000000000");
            h[156] = if data.is_some() { b'0' } else { b'5' };
            h[257..263].copy_from_slice(b"ustar\0");
            h[263..265].copy_from_slice(b"00");
            h[148..156].copy_from_slice(b"        ");
            let sum: u32 = h.iter().map(|&b| u32::from(b)).sum();
            h[148..155].copy_from_slice(format!("{sum:06o}\0").as_bytes());
            out.extend_from_slice(&h);
            if let Some(data) = data {
                out.extend_from_slice(data);
                out.resize(out.len().div_ceil(BLOCK) * BLOCK, 0);
            }
        }
        out.resize(out.len() + 2 * BLOCK, 0);
        out
    }

    #[test]
    fn a_tar_of_files_and_folders_unpacks() {
        let dir = crate::settings::temp_dir("untar");
        let tar = dir.join("a.tar");
        fs::write(
            &tar,
            tar_of(&[
                ("samples", None),
                ("instrument.json", Some(b"{}")),
                ("samples/a.flac", Some(&[7u8; 1000])),
            ]),
        )
        .unwrap();
        unpack(&tar, &dir.join("out")).unwrap();
        assert_eq!(fs::read(dir.join("out/instrument.json")).unwrap(), b"{}");
        assert_eq!(
            fs::read(dir.join("out/samples/a.flac")).unwrap(),
            [7u8; 1000]
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn a_name_that_leaves_the_folder_or_a_link_refuses_the_file() {
        let dir = crate::settings::temp_dir("untar-bad");
        for (i, name) in ["../x", "/etc/x", "a/../../x", "a\\b"].iter().enumerate() {
            let tar = dir.join(format!("{i}.tar"));
            fs::write(&tar, tar_of(&[(name, Some(b"x"))])).unwrap();
            assert!(
                unpack(&tar, &dir.join(format!("out{i}"))).is_err(),
                "{name}"
            );
        }
        assert!(!dir.join("x").exists());
        let mut link = tar_of(&[("l", Some(b""))]);
        link[156] = b'2';
        link[148..156].copy_from_slice(b"        ");
        let sum: u32 = link[..BLOCK].iter().map(|&b| u32::from(b)).sum();
        link[148..155].copy_from_slice(format!("{sum:06o}\0").as_bytes());
        fs::write(dir.join("link.tar"), &link).unwrap();
        assert!(unpack(&dir.join("link.tar"), &dir.join("outl")).is_err());
        // A changed byte in a header.
        let mut broken = tar_of(&[("a", Some(b"x"))]);
        broken[0] = b'b';
        fs::write(dir.join("sum.tar"), &broken).unwrap();
        assert!(unpack(&dir.join("sum.tar"), &dir.join("outs")).is_err());
        // Cut short.
        let whole = tar_of(&[("a", Some(&[1u8; 2000]))]);
        fs::write(dir.join("short.tar"), &whole[..1000]).unwrap();
        assert!(unpack(&dir.join("short.tar"), &dir.join("outc")).is_err());
        let _ = fs::remove_dir_all(dir);
    }
}
