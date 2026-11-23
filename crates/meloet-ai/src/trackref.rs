//! How a tool argument names a track: by its number from 1, as song_overview lists the
//! tracks now, or by its id, which stays the same while tracks are added, moved or deleted.

use std::borrow::Cow;
use std::fmt;
use std::sync::Arc;

use meloet_core::ai::AiSong;
use meloet_model::{Track, TrackId};
use serde::Deserialize;
use serde_json::Value;

/// A track, by number (`2`) or by id (`"id:7"`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrackRef {
    Number(u32),
    Id(u32),
}

const FORMS: &str = "a track is its number from 1 (like 2) or its id as text (like \"id:7\")";

/// The id as tools give it and take it back.
pub fn id_text(id: TrackId) -> String {
    format!("id:{}", id.0)
}

/// The id of the track of number `number` (from 1) in what was read.
pub fn id_of(read: &AiSong, number: u32) -> Option<String> {
    (number.checked_sub(1))
        .and_then(|i| read.song.doc.tracks.get(i as usize))
        .map(|t| id_text(t.id))
}

impl TrackRef {
    fn parse(value: &Value) -> Result<TrackRef, String> {
        match value {
            Value::Number(n) => (n.as_u64())
                .and_then(|n| u32::try_from(n).ok())
                .map(TrackRef::Number)
                .ok_or_else(|| FORMS.to_owned()),
            Value::String(text) => (text.strip_prefix("id:"))
                .filter(|digits| !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()))
                .and_then(|digits| digits.parse().ok())
                .map(TrackRef::Id)
                .ok_or_else(|| format!("\"{text}\" is not a track: {FORMS}")),
            _ => Err(FORMS.to_owned()),
        }
    }

    /// The track in what was read, and its number from 1.
    pub fn find(self, read: &AiSong) -> Result<(u32, &Arc<Track>), String> {
        let tracks = &read.song.doc.tracks;
        let found = match self {
            TrackRef::Number(n) => {
                (n.checked_sub(1)).and_then(|i| tracks.get(i as usize).map(|t| (i, t)))
            }
            TrackRef::Id(id) => (tracks.iter().enumerate())
                .find(|(_, t)| t.id.0 == id)
                .map(|(i, t)| (i as u32, t)),
        };
        found.map(|(i, t)| (i + 1, t)).ok_or_else(|| match self {
            TrackRef::Number(n) => {
                format!("there is no track {n}; call song_overview to see the tracks")
            }
            TrackRef::Id(id) => format!(
                "no track has the id \"id:{id}\"; it may have been deleted. Call song_overview \
                 to see the tracks"
            ),
        })
    }

    /// The track's number from 1 now.
    pub fn number(self, read: &AiSong) -> Result<u32, String> {
        self.find(read).map(|(n, _)| n)
    }
}

impl fmt::Display for TrackRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TrackRef::Number(n) => write!(f, "{n}"),
            TrackRef::Id(id) => write!(f, "id:{id}"),
        }
    }
}

impl<'de> Deserialize<'de> for TrackRef {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<TrackRef, D::Error> {
        TrackRef::parse(&Value::deserialize(d)?).map_err(serde::de::Error::custom)
    }
}

/// As it was given: the number, or the id's text.
impl serde::Serialize for TrackRef {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            TrackRef::Number(n) => s.serialize_u32(*n),
            TrackRef::Id(_) => s.collect_str(self),
        }
    }
}

impl schemars::JsonSchema for TrackRef {
    fn inline_schema() -> bool {
        true
    }

    fn schema_name() -> Cow<'static, str> {
        "TrackRef".into()
    }

    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "description": "A track: its number from 1 as song_overview lists it now (like 2), \
                or its id from song_overview (like \"id:7\"). Numbers shift when tracks are \
                added, moved or deleted, so a number read earlier may point at another track \
                later; an id always names the same track.",
            "anyOf": [
                { "type": "integer", "minimum": 1 },
                { "type": "string", "pattern": "^id:[0-9]+$" }
            ]
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn parse(v: Value) -> Result<TrackRef, String> {
        serde_json::from_value::<TrackRef>(v).map_err(|e| e.to_string())
    }

    #[test]
    fn both_forms() {
        assert_eq!(parse(json!(3)), Ok(TrackRef::Number(3)));
        assert_eq!(parse(json!("id:12")), Ok(TrackRef::Id(12)));
        assert_eq!(parse(json!("id:0")), Ok(TrackRef::Id(0)));
        // 0 is read; no track has it, which `find` says.
        assert_eq!(parse(json!(0)), Ok(TrackRef::Number(0)));
        assert_eq!(TrackRef::Id(12).to_string(), "id:12");
        assert_eq!(TrackRef::Number(3).to_string(), "3");
        assert_eq!(json!(TrackRef::Number(3)), json!(3));
        assert_eq!(json!(TrackRef::Id(12)), json!("id:12"));
    }

    #[test]
    fn bad_forms() {
        for bad in [
            json!(-1),
            json!(1.5),
            json!(4_294_967_296u64),
            json!("3"),
            json!("id:"),
            json!("id:x"),
            json!("id:-1"),
            json!("id: 3"),
            json!("ID:3"),
            json!("id:3 "),
            json!("id:99999999999"),
            json!(null),
            json!(true),
            json!([1]),
            json!({ "id": 3 }),
        ] {
            let e = parse(bad.clone()).unwrap_err();
            assert!(e.contains("\"id:7\""), "{bad}: {e}");
        }
    }

    fn read(song: meloet_model::Song) -> AiSong {
        AiSong {
            song,
            undo_count: 0,
            busy: None,
            library: Default::default(),
            file: None,
            recent: Vec::new(),
            songs_dir: None,
            unsaved: false,
            sample_rate: 48_000,
            outcomes: Vec::new(),
        }
    }

    #[test]
    fn found_by_number_and_by_id() {
        let mut song = meloet_model::Song::new_default();
        let first = song.doc.tracks[0].id;
        let second = TrackId(first.0 + 40);
        let mut other = (*song.doc.tracks[0]).clone();
        other.id = second;
        song.doc.tracks.insert(1, Arc::new(other));
        let r = read(song.clone());
        assert_eq!(TrackRef::Number(2).find(&r).unwrap().1.id, second);
        assert_eq!(TrackRef::Id(second.0).number(&r), Ok(2));
        assert_eq!(id_of(&r, 1), Some(id_text(first)));
        assert_eq!(id_of(&r, 0), None);
        let e = TrackRef::Number(0).find(&r).unwrap_err();
        assert!(e.starts_with("there is no track 0;"), "{e}");
        assert!(e.contains("song_overview"), "{e}");
        let past = r.song.doc.tracks.len() as u32 + 1;
        assert!(TrackRef::Number(past).find(&r).is_err());

        // The first track deleted: its id names nothing, the second is number 1 now.
        song.doc.tracks.remove(0);
        let r = read(song);
        let e = TrackRef::Id(first.0).find(&r).unwrap_err();
        assert!(
            e.contains(&format!("\"id:{}\"", first.0)) && e.contains("deleted"),
            "{e}"
        );
        assert_eq!(TrackRef::Id(second.0).number(&r), Ok(1));
    }

    #[test]
    fn in_a_list_both_forms_mix() {
        let list: Vec<TrackRef> = serde_json::from_value(json!([1, "id:4", 2])).unwrap();
        assert_eq!(
            list,
            [TrackRef::Number(1), TrackRef::Id(4), TrackRef::Number(2)]
        );
    }
}
