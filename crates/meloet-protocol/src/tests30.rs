//! The shapes protocol 30 added: the color themes (「表示 ▸ テーマ ▸」).

use serde_json::{Value, json};

use super::*;

fn to_json<T: serde::Serialize>(v: &T) -> Value {
    serde_json::to_value(v).unwrap()
}

fn both_ways<T>(v: T, json: Value)
where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    assert_eq!(to_json(&v), json);
    assert_eq!(serde_json::from_value::<T>(json).unwrap(), v);
}

#[test]
fn the_theme_names_are_those_of_the_screen() {
    // ui/src/design/themes.ts `THEME_IDS`, in the same order.
    let names = [
        "dark",
        "light",
        "pinkDark",
        "pinkLight",
        "mintDark",
        "mintLight",
    ];
    assert_eq!(Theme::ALL.map(Theme::name), names);
    for (theme, name) in Theme::ALL.into_iter().zip(names) {
        both_ways(theme, json!(name));
        assert_eq!(Theme::from_name(name), Some(theme));
    }
    assert_eq!(Theme::from_name("Dark"), None);
    assert_eq!(Theme::default(), Theme::Dark);
}

#[test]
fn the_new_command() {
    both_ways(
        Command::SetTheme {
            theme: Theme::PinkLight,
        },
        json!({"type": "setTheme", "theme": "pinkLight"}),
    );
    assert!(
        serde_json::from_value::<Command>(json!({"type": "setTheme", "theme": "blue"})).is_err()
    );
}

#[test]
fn the_theme_is_in_the_workspace() {
    let old = tests::sample_state();
    assert_eq!(to_json(&old)["workspace"]["theme"], json!("dark"));
    let mut new = old.clone();
    new.workspace.theme = Theme::MintDark;
    let changes = AppState::changes_from(&old, &new);
    assert_eq!(changes, vec![Change::Workspace(new.workspace.clone())]);
    assert_eq!(to_json(&changes[0])["value"]["theme"], json!("mintDark"));
}
