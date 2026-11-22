//! The catalog of the sound pack (docs/architecture.md 23-1): which parts the pack has, their
//! versions, sizes, sha256 and where to fetch them. [`check`] is the one place a catalog is
//! accepted; the signature (23-1) is to be checked there too.

use meloet_model::BuiltinInstrument;
use serde::Deserialize;

/// The catalog built into meloet (`tools/sounds/split.py --catalog`): the first fetch does not
/// wait for the one on the net.
pub const BUILT_IN: &str = include_str!("../../../../tools/sounds/catalog.json");
/// Where the newest catalog is kept on the net (23-3).
pub const ONLINE: &str =
    "https://raw.githubusercontent.com/wintermaples/meloet/main/sounds/catalog.json";
/// The only place a catalog may send meloet for its files.
pub const RELEASES: &str = "https://github.com/wintermaples/meloet/releases/download/";

const FORMAT: &str = "meloet-sounds-catalog";
const VERSION: u32 = 1;
pub const COMMON_ID: &str = "common";

/// One file of the pack.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Part {
    pub id: String,
    pub version: String,
    pub file: String,
    pub url: String,
    pub bytes: u64,
    pub sha256: String,
}

impl Part {
    /// The instrument it holds; `None` for the common part.
    pub fn instrument(&self) -> Option<BuiltinInstrument> {
        BuiltinInstrument::from_name(&self.id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Catalog {
    /// The pack's version, like `2026.12.0`.
    pub pack: String,
    /// The common part first, then the instruments in the catalog's order.
    pub parts: Vec<Part>,
}

impl Catalog {
    pub fn bytes(&self) -> u64 {
        self.parts.iter().map(|p| p.bytes).sum()
    }

    pub fn instruments(&self) -> impl Iterator<Item = BuiltinInstrument> + '_ {
        self.parts.iter().filter_map(Part::instrument)
    }

    pub fn part_of(&self, instrument: BuiltinInstrument) -> Option<&Part> {
        self.parts
            .iter()
            .find(|p| p.instrument() == Some(instrument))
    }

    /// Whether this one is newer than `other` (the numbers of the pack's version compared).
    pub fn newer_than(&self, other: &Catalog) -> bool {
        pack_numbers(&self.pack) > pack_numbers(&other.pack)
    }
}

fn pack_numbers(pack: &str) -> Vec<u64> {
    pack.split('.').map(|n| n.parse().unwrap_or(0)).collect()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OnDisk {
    format: String,
    version: u32,
    pack: String,
    common: PartOnDisk,
    instruments: Vec<PartOnDisk>,
}

#[derive(Deserialize)]
struct PartOnDisk {
    id: String,
    version: String,
    file: String,
    url: String,
    bytes: u64,
    sha256: String,
    #[serde(default)]
    #[allow(dead_code)]
    credits: Vec<String>,
}

/// What a catalog may name, beside its shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trust {
    /// Every url starts with this.
    pub urls_under: String,
}

impl Default for Trust {
    fn default() -> Self {
        Trust {
            urls_under: RELEASES.to_owned(),
        }
    }
}

fn plain(s: &str, max: usize) -> bool {
    !s.is_empty()
        && s.len() <= max
        && !s.starts_with('.')
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-' | b'_'))
}

/// What comes after the place the urls must be under: path segments of plain letters only,
/// so that no `..`, `%2e` or query leads somewhere else on the same host.
fn safe_path(rest: &str) -> bool {
    !rest.is_empty() && rest.split('/').all(|segment| plain(segment, 128))
}

/// Reads a catalog and accepts it only when everything in it is as meloet expects: the format,
/// names that are safe as file names, a sha256 and a size for each part, every instrument once
/// and known, every url under `trust.urls_under` and nothing else.
pub fn check(text: &str, trust: &Trust) -> Result<Catalog, String> {
    let disk: OnDisk = serde_json::from_str(text).map_err(|e| e.to_string())?;
    if disk.format != FORMAT || disk.version != VERSION {
        return Err(format!(
            "{} {} is not {FORMAT} {VERSION}",
            disk.format, disk.version
        ));
    }
    if !plain(&disk.pack, 32) || !disk.pack.bytes().all(|b| b.is_ascii_digit() || b == b'.') {
        return Err(format!("pack {:?}", disk.pack));
    }
    if disk.common.id != COMMON_ID {
        return Err(format!("common has the id {:?}", disk.common.id));
    }
    let mut parts = Vec::with_capacity(disk.instruments.len() + 1);
    for p in std::iter::once(disk.common).chain(disk.instruments) {
        let name = p.id.clone();
        let bad = |what: &str| Err(format!("{name}: {what}"));
        if parts.iter().any(|q: &Part| q.id == p.id) {
            return bad("twice");
        }
        if p.id != COMMON_ID {
            match BuiltinInstrument::from_name(&p.id) {
                Some(b) if crate::instruments::sampled(b) => {}
                _ => return bad("not an instrument with samples"),
            }
        }
        if !plain(&p.version, 64) || !plain(&p.file, 128) {
            return bad("version or file");
        }
        if p.sha256.len() != 64 || !p.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return bad("sha256");
        }
        if p.bytes == 0 {
            return bad("bytes");
        }
        if !p
            .url
            .strip_prefix(trust.urls_under.as_str())
            .is_some_and(safe_path)
        {
            return bad("url");
        }
        parts.push(Part {
            id: p.id,
            version: p.version,
            file: p.file,
            url: p.url,
            bytes: p.bytes,
            sha256: p.sha256.to_ascii_lowercase(),
        });
    }
    Ok(Catalog {
        pack: disk.pack,
        parts,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    pub(crate) fn sample(url: &str) -> String {
        format!(
            r#"{{"format": "meloet-sounds-catalog", "version": 1, "pack": "2026.12.0",
             "common": {{"id": "common", "version": "aa", "file": "common-aa.tar",
                         "url": "{url}common-aa.tar", "bytes": 10, "sha256": "{h}"}},
             "instruments": [{{"id": "grandPiano", "version": "bb", "file": "grandPiano-bb.tar",
                         "url": "{url}grandPiano-bb.tar", "bytes": 20, "sha256": "{h}",
                         "credits": ["salamander-grand-piano"]}}]}}"#,
            h = "ab".repeat(32)
        )
    }

    #[test]
    fn the_built_in_catalog_is_accepted() {
        let catalog = check(BUILT_IN, &Trust::default()).unwrap();
        assert_eq!(catalog.parts[0].id, COMMON_ID);
        let mut kinds: Vec<_> = catalog.instruments().collect();
        assert_eq!(kinds.len(), 26, "every instrument with samples");
        kinds.sort_by_key(|b| b.name());
        kinds.dedup();
        assert_eq!(kinds.len(), 26);
        assert!(catalog.bytes() > 100_000_000);
    }

    #[test]
    fn only_urls_under_the_releases_are_accepted() {
        let trust = Trust::default();
        let good = sample(RELEASES);
        let catalog = check(&good, &trust).unwrap();
        assert_eq!(catalog.parts.len(), 2);
        assert_eq!(
            catalog.parts[1].instrument(),
            Some(BuiltinInstrument::GrandPiano)
        );
        assert_eq!(catalog.bytes(), 30);
        for other in [
            "https://example.com/",
            "http://github.com/wintermaples/meloet/releases/download/",
            "https://github.com/someone/meloet/releases/download/",
            "https://github.com/wintermaples/meloet/releases/download/../../x/",
        ] {
            assert!(check(&sample(other), &trust).is_err(), "{other}");
        }
        for tail in [
            "grandPiano-bb.tar?x=1",
            "%2e%2e/x.tar",
            "a//b.tar",
            "./b.tar",
        ] {
            let text = good.replacen(
                "grandPiano-bb.tar\", \"bytes",
                &format!("{tail}\", \"bytes"),
                1,
            );
            assert_ne!(text, good);
            assert!(check(&text, &trust).is_err(), "{tail}");
        }
    }

    #[test]
    fn a_catalog_with_something_wrong_is_not_used() {
        let trust = Trust::default();
        let good = sample(RELEASES);
        for (from, to) in [
            ("meloet-sounds-catalog", "meloet-sounds"),
            ("\"version\": 1", "\"version\": 2"),
            ("\"grandPiano\"", "\"synthLead\""),
            ("\"grandPiano\"", "\"kazoo\""),
            ("\"grandPiano\"", "\"common\""),
            ("\"file\": \"grandPiano-bb.tar\"", "\"file\": \"../x.tar\""),
            ("\"version\": \"bb\"", "\"version\": \"b/b\""),
            ("\"bytes\": 20", "\"bytes\": 0"),
            ("\"pack\": \"2026.12.0\"", "\"pack\": \"../x\""),
        ] {
            let text = good.replacen(from, to, 1);
            assert_ne!(text, good, "{from}");
            assert!(check(&text, &trust).is_err(), "{from} -> {to}");
        }
        let short = good.replacen(&"ab".repeat(32), "abc", 1);
        assert!(check(&short, &trust).is_err());
        assert!(check("{", &trust).is_err());
    }

    #[test]
    fn newer_compares_the_numbers() {
        let a = check(&sample(RELEASES), &Trust::default()).unwrap();
        let mut b = a.clone();
        b.pack = "2026.12.10".into();
        assert!(b.newer_than(&a) && !a.newer_than(&b) && !a.newer_than(&a));
        b.pack = "2027.1.0".into();
        assert!(b.newer_than(&a));
    }
}
