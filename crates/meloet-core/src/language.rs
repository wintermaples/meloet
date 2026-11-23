//! 「ことば / Language」 before the person chose one (round 12, D4): the PC's language.

use meloet_protocol::Language;

/// The language of the PC as the locale variables say it, in the order that decides the
/// language of messages: `LC_ALL`, then `LC_MESSAGES`, then `LANG` (an empty one is passed
/// over). Japanese when it starts with `ja`, English otherwise (also when none is set).
pub fn of_locale(lc_all: Option<&str>, lc_messages: Option<&str>, lang: Option<&str>) -> Language {
    let first = [lc_all, lc_messages, lang]
        .into_iter()
        .flatten()
        .find(|v| !v.is_empty());
    match first {
        Some(v) if v.starts_with("ja") => Language::Ja,
        _ => Language::En,
    }
}

/// [`of_locale`] of this process's environment.
pub fn of_pc() -> Language {
    let var = |name| std::env::var(name).ok();
    of_locale(
        var("LC_ALL").as_deref(),
        var("LC_MESSAGES").as_deref(),
        var("LANG").as_deref(),
    )
}

/// The language the names of new songs and tracks are made in.
pub fn names(language: Language) -> meloet_model::Language {
    match language {
        Language::Ja => meloet_model::Language::Ja,
        Language::En => meloet_model::Language::En,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_variable_set_decides() {
        let ja = Some("ja_JP.UTF-8");
        let en = Some("en_US.UTF-8");
        assert_eq!(of_locale(ja, None, None), Language::Ja);
        assert_eq!(of_locale(None, ja, en), Language::Ja);
        assert_eq!(of_locale(None, None, ja), Language::Ja);
        assert_eq!(of_locale(en, ja, ja), Language::En);
        assert_eq!(of_locale(None, en, ja), Language::En);
        assert_eq!(of_locale(Some(""), Some(""), ja), Language::Ja);
        assert_eq!(of_locale(Some("ja"), None, None), Language::Ja);
        assert_eq!(of_locale(Some("C"), None, ja), Language::En);
        assert_eq!(of_locale(Some("fr_FR.UTF-8"), None, None), Language::En);
        assert_eq!(of_locale(None, None, None), Language::En);
    }
}
