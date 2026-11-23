//! 「曲の情報」 of an export (round 12 F2): the title, the artist and the picture, written
//! into a finished file of the work place before it is moved (lofty 0.25.5). An mp3 holds
//! an ID3v2 tag, a flac Vorbis comments and a picture block, a wav a RIFF INFO list (the
//! title and the artist: a wav holds no picture).

use std::fs::OpenOptions;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::sync::Arc;

use lofty::config::WriteOptions;
use lofty::picture::{MimeType, Picture, PictureInformation, PictureType};
use lofty::tag::{Accessor, Tag, TagExt, TagType};
use meloet_protocol::{EXPORT_PICTURE_MAX_BYTES, ExportFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureKind {
    Png,
    Jpeg,
}

/// A picture chosen for 「絵」, as it was read when it was chosen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportPicture {
    /// The file's name without its folder: what the dialog shows.
    pub name: String,
    pub kind: PictureKind,
    pub data: Arc<[u8]>,
}

/// Why a picture that was chosen is not taken.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PictureProblem {
    /// More than `EXPORT_PICTURE_MAX_BYTES`.
    TooLarge,
    /// Not a png or a jpg whose head reads, or the file does not read.
    Unreadable,
}

/// What the files of an export say of themselves. Empty text is not written.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SongInfo {
    pub title: String,
    pub artist: String,
    pub picture: Option<ExportPicture>,
}

impl SongInfo {
    /// For the file of one track of 「トラックごと」: 「題名 - トラックの名前」.
    pub fn for_track(&self, track: &str) -> SongInfo {
        let title = match (self.title.trim(), track.trim()) {
            ("", track) => track.to_owned(),
            (title, "") => title.to_owned(),
            (title, track) => format!("{title} - {track}"),
        };
        SongInfo {
            title,
            ..self.clone()
        }
    }

    fn is_empty(&self) -> bool {
        self.title.trim().is_empty() && self.artist.trim().is_empty() && self.picture.is_none()
    }
}

/// Reads the picture at `path` for 「絵」: a png or a jpg of at most
/// `EXPORT_PICTURE_MAX_BYTES`. It reads up to that many bytes: not on a thread that must
/// not wait.
pub fn read_picture(path: &Path) -> Result<ExportPicture, PictureProblem> {
    let name = (path.file_name()).map_or_else(String::new, |n| n.to_string_lossy().into_owned());
    let file = std::fs::File::open(path).map_err(|_| PictureProblem::Unreadable)?;
    let mut data = Vec::new();
    // One byte more than may be taken: a file that grows while it is read is too large too.
    (file.take(EXPORT_PICTURE_MAX_BYTES + 1))
        .read_to_end(&mut data)
        .map_err(|_| PictureProblem::Unreadable)?;
    if data.len() as u64 > EXPORT_PICTURE_MAX_BYTES {
        return Err(PictureProblem::TooLarge);
    }
    let kind = if PictureInformation::from_png(&data).is_ok() {
        PictureKind::Png
    } else if PictureInformation::from_jpeg(&data).is_ok() {
        PictureKind::Jpeg
    } else {
        return Err(PictureProblem::Unreadable);
    };
    Ok(ExportPicture {
        name,
        kind,
        data: data.into(),
    })
}

fn tag_failed(e: impl std::fmt::Display) -> io::Error {
    io::Error::other(format!("song information: {e}"))
}

/// The tag of `info` for a file of `format`.
fn tag_of(format: ExportFormat, info: &SongInfo) -> Tag {
    let mut tag = Tag::new(match format {
        ExportFormat::Mp3 => TagType::Id3v2,
        ExportFormat::Flac => TagType::VorbisComments,
        ExportFormat::Wav => TagType::RiffInfo,
    });
    if !info.title.trim().is_empty() {
        tag.set_title(info.title.clone());
    }
    if !info.artist.trim().is_empty() {
        tag.set_artist(info.artist.clone());
    }
    if let Some(picture) = info
        .picture
        .as_ref()
        .filter(|_| format != ExportFormat::Wav)
    {
        let mime = match picture.kind {
            PictureKind::Png => MimeType::Png,
            PictureKind::Jpeg => MimeType::Jpeg,
        };
        tag.push_picture(
            Picture::unchecked(picture.data.to_vec())
                .pic_type(PictureType::CoverFront)
                .mime_type(mime)
                .build(),
        );
    }
    tag
}

/// Writes `info` into the finished file at `path`, which is of `format`. Nothing is written
/// for empty information.
pub fn write(path: &Path, format: ExportFormat, info: &SongInfo) -> io::Result<()> {
    if info.is_empty() {
        return Ok(());
    }
    let tag = tag_of(format, info);
    match format {
        ExportFormat::Wav => append_riff_info(path, &tag),
        ExportFormat::Mp3 | ExportFormat::Flac => {
            (tag.save_to_path(path, WriteOptions::default())).map_err(tag_failed)?;
            std::fs::File::open(path)?.sync_all()
        }
    }
}

/// A RIFF INFO list after the data of a wav as `crate::audiofiles::wav::wav_head` writes it,
/// and the RIFF size made to count it. lofty makes the list; its own way of putting it in
/// reads the whole file into memory.
fn append_riff_info(path: &Path, tag: &Tag) -> io::Result<()> {
    let mut list = Vec::new();
    (tag.dump_to(&mut list, WriteOptions::default())).map_err(tag_failed)?;
    if list.is_empty() {
        return Ok(());
    }
    let mut file = OpenOptions::new().read(true).write(true).open(path)?;
    let mut head = [0u8; 12];
    file.read_exact(&mut head)?;
    if &head[..4] != b"RIFF" || &head[8..] != b"WAVE" {
        return Err(tag_failed("not a wav"));
    }
    let riff = u32::from_le_bytes([head[4], head[5], head[6], head[7]]);
    let end = file.seek(SeekFrom::End(0))?;
    // A wav too long to say its length (`u32::MAX`) gets no list.
    let Some(grown) = (u64::from(riff) + list.len() as u64)
        .try_into()
        .ok()
        .filter(|_| u64::from(riff) + 8 == end)
        .filter(|g: &u32| *g < u32::MAX)
    else {
        return Ok(());
    };
    file.write_all(&list)?;
    file.seek(SeekFrom::Start(4))?;
    file.write_all(&u32::to_le_bytes(grown))?;
    file.sync_all()
}

#[cfg(test)]
pub(crate) mod tests;
