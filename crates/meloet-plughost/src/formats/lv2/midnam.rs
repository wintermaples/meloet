//! The names of a plugin's keys, from Ardour's midnam extension
//! (`http://ardour.org/lv2/midnam#interface`): the plugin gives a MIDINameDocument (MMA MIDNAM
//! 1.0) as text, and only its note name lists are read here.

use std::ffi::{CStr, c_char, c_void};

use meloet_plugin::wire::KeyName;

pub const INTERFACE: &CStr = c"http://ardour.org/lv2/midnam#interface";

/// No document longer than this is read.
const MAX_DOCUMENT: usize = 1 << 20;
/// Characters of a name kept.
const MAX_NAME: usize = 128;

/// `LV2_Midnam_Interface`: the strings come from the plugin and go back to its `free`.
#[repr(C)]
pub struct Interface {
    pub midnam: Option<unsafe extern "C" fn(instance: *mut c_void) -> *mut c_char>,
    pub model: Option<unsafe extern "C" fn(instance: *mut c_void) -> *mut c_char>,
    pub free: Option<unsafe extern "C" fn(text: *mut c_char)>,
}

/// The plugin's MIDNAM document, copied (empty when it gives none or one too long); `None`
/// when it has no midnam interface. Read it with [`note_names`].
///
/// # Safety
/// `interface` is what the plugin's `extension_data` gave for [`INTERFACE`] (or null), and
/// `instance` its `LV2_Handle`. Nothing else may be in the plugin meanwhile.
pub unsafe fn document(interface: *const c_void, instance: *mut c_void) -> Option<String> {
    // SAFETY: the plugin gave this pointer for this interface; it lives as long as its library.
    let interface = unsafe { interface.cast::<Interface>().as_ref() }?;
    let midnam = interface.midnam?;
    // SAFETY: the plugin's own function with its own handle.
    let text = unsafe { midnam(instance) };
    if text.is_null() {
        return Some(String::new());
    }
    // SAFETY: the interface says the text ends with a NUL; no more than one byte past the
    // longest document read is looked at.
    let len = unsafe { libc::strnlen(text, MAX_DOCUMENT + 1) };
    let document = if len <= MAX_DOCUMENT {
        // SAFETY: `len` bytes before the NUL.
        let bytes = unsafe { std::slice::from_raw_parts(text.cast::<u8>(), len) };
        String::from_utf8_lossy(bytes).into_owned()
    } else {
        String::new()
    };
    if let Some(free) = interface.free {
        // SAFETY: the text came from `midnam` and is given back once.
        unsafe { free(text) };
    }
    Some(document)
}

/// [`note_names`] as the wire carries them.
pub fn key_names(document: &str) -> Vec<KeyName> {
    (note_names(document).into_iter())
        .map(|(key, name)| KeyName { key, name })
        .collect()
}

/// A tag of the document: its name and the text of its attributes.
struct Tag<'a> {
    name: &'a str,
    attributes: &'a str,
    /// `</name>`.
    closing: bool,
    /// `<name … />`.
    alone: bool,
}

/// The tags of `xml` in order; comments, CDATA, declarations and the DOCTYPE are passed over.
fn tags(xml: &str) -> impl Iterator<Item = Tag<'_>> {
    let mut rest = xml;
    std::iter::from_fn(move || {
        loop {
            let at = rest.find('<')?;
            rest = &rest[at..];
            let skip_to = |end: &str, rest: &mut &str| match rest.find(end) {
                Some(i) => *rest = &rest[i + end.len()..],
                None => *rest = "",
            };
            if rest.starts_with("<!--") {
                skip_to("-->", &mut rest);
                continue;
            }
            if rest.starts_with("<![CDATA[") {
                skip_to("]]>", &mut rest);
                continue;
            }
            if rest.starts_with("<?") {
                skip_to("?>", &mut rest);
                continue;
            }
            if rest.starts_with("<!") {
                // A DOCTYPE may hold an internal subset in brackets, with '>' inside.
                let close = rest.find('>').unwrap_or(rest.len());
                if rest[..close].contains('[') {
                    skip_to("]>", &mut rest);
                } else {
                    skip_to(">", &mut rest);
                }
                continue;
            }
            // The end of the tag: the first '>' outside quotes (a value may hold '>').
            let mut quote = None;
            let end = rest
                .char_indices()
                .skip(1)
                .find_map(|(i, c)| match (quote, c) {
                    (None, '"' | '\'') => {
                        quote = Some(c);
                        None
                    }
                    (Some(q), c) if c == q => {
                        quote = None;
                        None
                    }
                    (None, '>') => Some(i),
                    _ => None,
                });
            let Some(end) = end else {
                rest = "";
                return None;
            };
            let inside = &rest[1..end];
            rest = &rest[end + 1..];
            let (closing, inside) = match inside.strip_prefix('/') {
                Some(inside) => (true, inside),
                None => (false, inside),
            };
            let (alone, inside) = match inside.trim_end().strip_suffix('/') {
                Some(inside) => (true, inside),
                None => (false, inside),
            };
            let split = inside
                .find(|c: char| c.is_whitespace())
                .unwrap_or(inside.len());
            return Some(Tag {
                name: &inside[..split],
                attributes: &inside[split..],
                closing,
                alone,
            });
        }
    })
}

/// The value of the attribute `name`, with its entities read.
fn attribute(attributes: &str, name: &str) -> Option<String> {
    let mut rest = attributes;
    loop {
        rest = rest.trim_start();
        let eq = rest.find('=')?;
        let key = rest[..eq].trim();
        let after = rest[eq + 1..].trim_start();
        let quote = after.chars().next().filter(|c| matches!(c, '"' | '\''))?;
        let close = after[1..].find(quote)? + 1;
        if key == name {
            return Some(entities(&after[1..close]));
        }
        rest = &after[close + 1..];
    }
}

/// `&amp;`, `&lt;`, `&gt;`, `&quot;`, `&apos;` and character references; others stay as
/// they are.
fn entities(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find('&') {
        out.push_str(&rest[..at]);
        rest = &rest[at..];
        let Some(semi) = rest.find(';').filter(|&semi| semi <= 10) else {
            out.push('&');
            rest = &rest[1..];
            continue;
        };
        let entity = &rest[1..semi];
        let read = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ => entity
                .strip_prefix("#x")
                .or_else(|| entity.strip_prefix("#X"))
                .map(|hex| u32::from_str_radix(hex, 16))
                .or_else(|| entity.strip_prefix('#').map(str::parse::<u32>))
                .and_then(Result::ok)
                .and_then(char::from_u32),
        };
        match read {
            Some(c) => {
                out.push(c);
                rest = &rest[semi + 1..];
            }
            None => {
                out.push('&');
                rest = &rest[1..];
            }
        }
    }
    out.push_str(rest);
    out
}

/// The names of the note name list the document says is used (its first `UsesNoteNameList`),
/// else of its first list that names a key; lowest key first. A key named twice in that list
/// keeps its first name; empty names and numbers outside 0..=127 are passed over.
pub fn note_names(xml: &str) -> Vec<(u8, String)> {
    /// A list's name and its notes.
    type List = (Option<String>, Vec<(u8, String)>);
    let mut used: Option<String> = None;
    let mut lists: Vec<List> = Vec::new();
    let mut inside = false;
    for tag in tags(xml) {
        match (tag.name, tag.closing) {
            ("UsesNoteNameList", false) if used.is_none() => {
                used = attribute(tag.attributes, "Name");
            }
            ("NoteNameList", false) => {
                lists.push((attribute(tag.attributes, "Name"), Vec::new()));
                inside = !tag.alone;
            }
            ("NoteNameList", true) => inside = false,
            ("Note", false) if inside => {
                let number = attribute(tag.attributes, "Number")
                    .and_then(|n| n.trim().parse::<u8>().ok())
                    .filter(|n| *n <= 127);
                let name = attribute(tag.attributes, "Name");
                if let (Some(key), Some(name), Some((_, notes))) = (number, name, lists.last_mut())
                {
                    let name: String = name.trim().chars().take(MAX_NAME).collect();
                    if !name.is_empty() && !notes.iter().any(|(k, _)| *k == key) {
                        notes.push((key, name));
                    }
                }
            }
            _ => {}
        }
    }
    let chosen = used
        .and_then(|used| {
            lists
                .iter()
                .position(|(name, _)| name.as_deref() == Some(&used))
        })
        .or_else(|| lists.iter().position(|(_, notes)| !notes.is_empty()));
    let mut names = chosen.map(|at| lists.swap_remove(at).1).unwrap_or_default();
    names.sort_by_key(|(key, _)| *key);
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    /// As avldrums 0.7.3 gives it for Black Pearl (shortened).
    const AVL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE MIDINameDocument PUBLIC "-//MIDI Manufacturers Association//DTD MIDINameDocument 1.0//EN" "http://www.midi.org/dtds/MIDINameDocument10.dtd">
<MIDINameDocument>
  <Author>Glen MacArthur</Author>
  <MasterDeviceNames>
    <Manufacturer>AVL-Drumkits</Manufacturer>
    <Model>AVL-Drumkits-LV2:0x5555</Model>
    <CustomDeviceMode Name="Drumkit Keymap">
      <ChannelNameSetAssignments>
        <ChannelNameSetAssign Channel="1" NameSet="Drumkit"/>
      </ChannelNameSetAssignments>
    </CustomDeviceMode>
    <ChannelNameSet Name="Drumkit">
      <AvailableForChannels>
        <AvailableChannel Channel="1" Available="true"/>
      </AvailableForChannels>
      <UsesNoteNameList Name="Notes"/>
    </ChannelNameSet>
    <NoteNameList Name="Notes">
      <Note Number="36" Name="Kick Drum"/>
      <Note Number="38" Name="Snare Center"/>
      <Note Number="42" Name="Hi-Hat Closed"/>
    </NoteNameList>
   </MasterDeviceNames>
</MIDINameDocument>
"#;

    fn names(xml: &str) -> Vec<(u8, String)> {
        note_names(xml)
    }

    fn owned(names: &[(u8, &str)]) -> Vec<(u8, String)> {
        names.iter().map(|(k, n)| (*k, n.to_string())).collect()
    }

    #[test]
    fn the_names_of_a_drum_plugin() {
        assert_eq!(
            names(AVL),
            owned(&[
                (36, "Kick Drum"),
                (38, "Snare Center"),
                (42, "Hi-Hat Closed")
            ])
        );
    }

    #[test]
    fn the_list_in_use_is_chosen_and_groups_are_inside_it() {
        let xml = r#"<MIDINameDocument>
  <!-- <NoteNameList Name="Commented"><Note Number="1" Name="No"/></NoteNameList> -->
  <NoteNameList Name='First'><Note Number='60' Name='Not this one'/></NoteNameList>
  <ChannelNameSet Name="Set"><UsesNoteNameList Name="Second"/></ChannelNameSet>
  <NoteNameList Name="Second">
    <NoteGroup Name="Toms">
      <Note Number = "50" Name = "Tom &amp; Rim &gt; 1"/>
      <Note Number="47" Name="Mid Tom"></Note>
    </NoteGroup>
    <Note Number="36" Name="  Kick  "/>
    <Note Number="36" Name="Kick again"/>
    <Note Number="128" Name="Too high"/>
    <Note Number="-1" Name="Too low"/>
    <Note Number="40" Name=""/>
    <Note Number="41" Name="Kick > Snare &#x41;&#66;"/>
  </NoteNameList>
</MIDINameDocument>"#;
        assert_eq!(
            names(xml),
            owned(&[
                (36, "Kick"),
                (41, "Kick > Snare AB"),
                (47, "Mid Tom"),
                (50, "Tom & Rim > 1")
            ])
        );
    }

    #[test]
    fn without_a_list_in_use_the_first_that_names_a_key() {
        let xml = r#"<NoteNameList Name="Empty"/><NoteNameList Name="B">
            <Note Number="35" Name="Kick 2"/></NoteNameList>"#;
        assert_eq!(names(xml), owned(&[(35, "Kick 2")]));
        assert_eq!(names("<MIDINameDocument/>"), []);
        assert_eq!(names("not xml at all < > &"), []);
        assert_eq!(names("<NoteNameList><Note Number=\"36\" Name=\"cut"), []);
    }

    #[test]
    fn entities_are_read() {
        assert_eq!(entities("a&lt;b&quot;c&apos;&#233;"), "a<b\"c'é");
        assert_eq!(entities("&unknown; & &#xZZ;"), "&unknown; & &#xZZ;");
    }

    #[test]
    fn a_plugin_without_the_interface_names_nothing() {
        // SAFETY: a null interface is never followed.
        let none = unsafe { document(std::ptr::null(), std::ptr::null_mut()) };
        assert_eq!(none, None);
        let silent = Interface {
            midnam: Some(nothing),
            model: None,
            free: None,
        };
        // SAFETY: the interface above; the instance is not looked at.
        let empty = unsafe { document((&raw const silent).cast(), std::ptr::null_mut()) };
        assert_eq!(empty.as_deref(), Some(""));
    }

    static FREED: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

    unsafe extern "C" fn nothing(_: *mut c_void) -> *mut c_char {
        std::ptr::null_mut()
    }

    unsafe extern "C" fn avl(_: *mut c_void) -> *mut c_char {
        let text = std::ffi::CString::new(AVL).unwrap();
        text.into_raw()
    }

    unsafe extern "C" fn free(text: *mut c_char) {
        // SAFETY: made by `avl` with `CString::into_raw`.
        drop(unsafe { std::ffi::CString::from_raw(text) });
        FREED.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    #[test]
    fn the_document_is_asked_for_and_given_back() {
        let interface = Interface {
            midnam: Some(avl),
            model: None,
            free: Some(free),
        };
        // SAFETY: the interface above; the instance is not looked at.
        let text = unsafe { document((&raw const interface).cast(), std::ptr::null_mut()) };
        assert_eq!(text.as_deref(), Some(AVL));
        assert_eq!(FREED.load(std::sync::atomic::Ordering::Relaxed), 1);
    }
}
