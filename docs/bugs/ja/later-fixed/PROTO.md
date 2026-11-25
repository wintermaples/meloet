# PROTO — 2 回目で直した点 (2026-11-25)

- crates/meloet-protocol/src/timeline.rs:3 — モジュールのコメントでは、テンポは拍子の下の数の音符で数えた拍を毎分いくつ進めるかと書かれています。CLAUDE.md と他のコードでは、拍子に関係なく毎分の四分音符の数です。契約の中にある、誤解を招くコメントです。
- crates/meloet-protocol/src/lib.rs:754 — `Command::Audition` には、画面が `AuditionOff` を送ると書かれています。しかし実際は、空の `keys` を付けた `audition` を送ります(`ui/src/components/pianoroll/audition.ts:78`)。そのため `Command::AuditionOff` を送る人はいません。
- crates/meloet-protocol/src/lib.rs:1527 — `Command::TypedKey` には、画面がキーの時刻を Rust の時計に直すのに `HelloReply::mono_us` を使うと書かれています。実際は `clockOffsetUs` を使います(`ui/src/keys/typed.svelte.ts:69`)。`HelloReply.monoUs` は、どこからも読まれていません。
- crates/meloet-protocol/src/lib.rs:3824 — `StatusMessage::AudioImported.skipped`(と `count`)は表示されていません。選んだファイルの一部が読めないとき、状態行には最初に取り込んだファイルの名前だけが出て、ファイルが抜けたことは知らされません。仕様 6 にその文はありません(文面は、オーナーさんに決めてもらう必要があります)。
