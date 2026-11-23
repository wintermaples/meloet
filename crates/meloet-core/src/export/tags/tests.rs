use std::path::Path;

use lofty::file::TaggedFileExt;
use lofty::tag::{Accessor, TagType};
use meloet_protocol::{EXPORT_PICTURE_MAX_BYTES, ExportFormat, RecordFormat};

use super::*;
use crate::audiofiles::decode::Decoder;
use crate::audiofiles::wav::WavPcm;
use crate::export::sink::{self, SinkSpec};
use crate::pcm::PcmFile;
use crate::settings::temp_dir;

/// The head of a picture of 16 × 16 of `kind`, as much as the tags need of it, and
/// `rest` bytes after it.
fn picture_bytes(kind: PictureKind, rest: usize) -> Vec<u8> {
    let mut bytes = match kind {
        PictureKind::Png => {
            let mut b = vec![0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];
            b.extend([0, 0, 0, 13]);
            b.extend(b"IHDR");
            b.extend(16u32.to_be_bytes());
            b.extend(16u32.to_be_bytes());
            b.extend([8, 2, 0, 0, 0]);
            b.extend([0x90, 0x91, 0x68, 0x36]);
            b
        }
        PictureKind::Jpeg => {
            let mut b = vec![0xFF, 0xD8, 0xFF, 0xE0, 0x00, 0x10];
            b.extend(b"JFIF\0");
            b.extend([1, 1, 0, 0, 1, 0, 1, 0, 0]);
            b.extend([0xFF, 0xC0, 0x00, 0x11, 8, 0, 16, 0, 16, 3]);
            b.extend([1, 0x22, 0, 2, 0x11, 1, 3, 0x11, 1]);
            b.extend([0xFF, 0xDA, 0x00, 0x0C]);
            b
        }
    };
    bytes.resize(bytes.len() + rest, 0);
    bytes
}

/// A picture as [`read_picture`] gives it, for the tests of the export.
pub(crate) fn picture(kind: PictureKind) -> ExportPicture {
    ExportPicture {
        name: match kind {
            PictureKind::Png => "表紙.png".into(),
            PictureKind::Jpeg => "表紙.jpg".into(),
        },
        kind,
        data: picture_bytes(kind, 64).into(),
    }
}

#[test]
fn a_png_or_a_jpg_up_to_5_mib_is_taken_and_anything_else_is_not() {
    let dir = temp_dir("export-picture");
    let at = |name: &str, bytes: &[u8]| {
        let path = dir.join(name);
        std::fs::write(&path, bytes).unwrap();
        path
    };
    let png = at("表紙.png", &picture_bytes(PictureKind::Png, 100));
    let taken = read_picture(&png).unwrap();
    assert_eq!(
        (taken.name.as_str(), taken.kind, taken.data.len()),
        ("表紙.png", PictureKind::Png, 133)
    );
    // The ending does not decide: the bytes do.
    let jpg = at("photo.PNG", &picture_bytes(PictureKind::Jpeg, 10));
    assert_eq!(read_picture(&jpg).unwrap().kind, PictureKind::Jpeg);

    let head = picture_bytes(PictureKind::Png, 0).len();
    let max = EXPORT_PICTURE_MAX_BYTES as usize;
    let just = at("just.png", &picture_bytes(PictureKind::Png, max - head));
    assert_eq!(read_picture(&just).unwrap().data.len(), max);
    let over = at("over.png", &picture_bytes(PictureKind::Png, max - head + 1));
    assert_eq!(read_picture(&over), Err(PictureProblem::TooLarge));

    let gif = at("a.gif", b"GIF89a\x10\x00\x10\x00\x00\x00\x00");
    assert_eq!(read_picture(&gif), Err(PictureProblem::Unreadable));
    let cut = at("cut.png", &picture_bytes(PictureKind::Png, 0)[..12]);
    assert_eq!(read_picture(&cut), Err(PictureProblem::Unreadable));
    let empty = at("empty.jpg", b"");
    assert_eq!(read_picture(&empty), Err(PictureProblem::Unreadable));
    assert_eq!(
        read_picture(&dir.join("gone.png")),
        Err(PictureProblem::Unreadable)
    );
    assert_eq!(read_picture(&dir), Err(PictureProblem::Unreadable));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_track_of_tracks_is_titled_after_the_song() {
    let info = SongInfo {
        title: "はじめての曲".into(),
        artist: "わたし".into(),
        picture: None,
    };
    assert_eq!(info.for_track("ベース").title, "はじめての曲 - ベース");
    assert_eq!(info.for_track("ベース").artist, "わたし");
    let untitled = SongInfo {
        title: " ".into(),
        ..info.clone()
    };
    assert_eq!(untitled.for_track("ベース").title, "ベース");
    assert_eq!(info.for_track("").title, "はじめての曲");
}

fn spec(format: ExportFormat) -> SinkSpec {
    SinkSpec {
        format,
        sample_rate: 48_000,
        channels: 2,
        bits: RecordFormat::Bits24,
    }
}

fn audio() -> Vec<f32> {
    (0..2 * 10_000)
        .map(|i| ((i / 2) as f32 * 0.03).sin() * 0.5)
        .collect()
}

fn written(path: &Path, format: ExportFormat) {
    let mut out = sink::create(path, spec(format)).unwrap();
    out.write(&audio()).unwrap();
    out.finish().unwrap();
}

fn decoded(path: &Path) -> Vec<f32> {
    let mut file = Decoder::open(path).unwrap();
    let mut out = Vec::new();
    while let Some((_, block)) = file.next_block().unwrap() {
        out.extend_from_slice(block);
    }
    out
}

fn info(picture: Option<ExportPicture>) -> SongInfo {
    SongInfo {
        title: "はじめての曲".into(),
        artist: "わたし".into(),
        picture,
    }
}

#[test]
fn a_flac_says_its_title_artist_and_picture_and_holds_the_same_samples() {
    let dir = temp_dir("export-tags-flac");
    let path = dir.join("a.flac");
    written(&path, ExportFormat::Flac);
    let before = decoded(&path);
    let picture = picture(PictureKind::Jpeg);
    write(&path, ExportFormat::Flac, &info(Some(picture.clone()))).unwrap();

    let file = lofty::read_from_path(&path).unwrap();
    let tag = file.tag(TagType::VorbisComments).unwrap();
    assert_eq!(tag.title().as_deref(), Some("はじめての曲"));
    assert_eq!(tag.artist().as_deref(), Some("わたし"));
    assert_eq!(tag.pictures().len(), 1);
    assert_eq!(tag.pictures()[0].data(), &picture.data[..]);
    assert_eq!(decoded(&path), before);
    assert_eq!(Decoder::open(&path).unwrap().header_frames(), Some(10_000));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn a_wav_says_its_title_and_artist_and_holds_the_same_samples() {
    let dir = temp_dir("export-tags-wav");
    let path = dir.join("a.wav");
    written(&path, ExportFormat::Wav);
    let bytes = std::fs::read(&path).unwrap();
    // A picture is not for a wav.
    write(
        &path,
        ExportFormat::Wav,
        &info(Some(picture(PictureKind::Png))),
    )
    .unwrap();

    let file = lofty::read_from_path(&path).unwrap();
    let tag = file.tag(TagType::RiffInfo).unwrap();
    assert_eq!(tag.title().as_deref(), Some("はじめての曲"));
    assert_eq!(tag.artist().as_deref(), Some("わたし"));
    assert!(tag.pictures().is_empty());
    // The audio is as it was, and the list comes after it.
    let now = std::fs::read(&path).unwrap();
    assert!(now.len() > bytes.len());
    assert_eq!(now[8..bytes.len()], bytes[8..]);
    let riff = u32::from_le_bytes(now[4..8].try_into().unwrap());
    assert_eq!(u64::from(riff) + 8, now.len() as u64);
    let mut wav = WavPcm::open(&path).unwrap();
    assert_eq!(wav.frames(), 10_000);
    let mut samples = vec![0.0; 20_000];
    wav.read(0, &mut samples).unwrap();
    assert_eq!(samples, decoded(&path));
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn nothing_is_written_for_no_information() {
    let dir = temp_dir("export-tags-none");
    for (name, format) in [("a.wav", ExportFormat::Wav), ("a.flac", ExportFormat::Flac)] {
        let path = dir.join(name);
        written(&path, format);
        let before = std::fs::read(&path).unwrap();
        let blank = SongInfo {
            title: "  ".into(),
            ..SongInfo::default()
        };
        write(&path, format, &blank).unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
    let _ = std::fs::remove_dir_all(dir);
}
