---
id: EDGE-001
title: シェル自身の拡大率の購読が画面のあいさつとして数えられ、プラグインのスキャンと渡された曲がページの読みこみ前に始まる
area: EDGE
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# EDGE-001: シェル自身の拡大率の購読が画面のあいさつとして数えられ、プラグインのスキャンと渡された曲がページの読みこみ前に始まる

## Where

- crates/meloet-app/src/main.rs:251-276 (`follow_scale` が `core.subscribe(...)` を呼ぶ)
- crates/meloet-app/src/main.rs:557-560 (`follow_scale` は Tauri の `setup` フックで動く)
- crates/meloet-core/src/service.rs:888-891 (`Core::subscribe` は必ず `Msg::ScreenReady` を送る)
- crates/meloet-core/src/service.rs:2844-2849 (`ScreenReady` は保留中のスキャン作業を解放し、`Input::ScreenUp` を与える)

## What happens

起動時、シェルは最小のウィンドウサイズのために拡大率を追うコアの購読を自分で行います。`Core::subscribe` は、どの購読も「画面のページが表示された」と扱い、`Msg::ScreenReady` を送ります。そのため `ScreenReady` は、Tauri の `setup` フックの中でコアのスレッドに届きます。これは WebView がページの読みこみを始める前で、ページ自身のあいさつ(`hello` → `subscribe`)よりずっと前です。

画面を待つはずの作業は、そのため待ちません。
- `PluginSide::held` が保持していたプラグインのスキャン作業が、すぐにスキャナーへ渡されます(WebKit がライブラリを読みこんでいる間に、何百もの子プロセスが始まります)。
- `Input::ScreenUp` は `songs_screen_up`(PC から渡された曲、`meloet song.meloet`、をすぐに開く)、`pack_screen_up`(カタログの確認と再開する取得)、`updates_screen_up` を動かします。
10 秒の代替待ち(`SCAN_WAITS_FOR_SCREEN`)は、解放がすぐに来るため一度も効きません。

## What should happen

スキャン、渡された曲、その他「画面が表示された」あとの作業は、ページのあいさつのあとに始まるべきです。docs/architecture.md:850 には「スキャンは画面の hello(`subscribe`)のあと、または 10 秒後に始まる … 理由: VS Code の CodeLLDB の下で、同時に起動すると meloet が止まり、ウィンドウが出なかった。WebKit のライブラリ読みこみと重なる起動は一時停止に見えた。ウィンドウも早く出る」とあります。CLAUDE.md(「この PC のこと」)には「子プロセスを一つずつ起動し、スキャンは画面のあいさつのあとに始めることで直した(2026-11-19)。どちらかを外すと、また起きる」とあります。docs/architecture.md:2614 には「画面のあいさつより前に来た経路は `Input::ScreenUp` まで保留される」とあります。コミット aa16851 にも同じことが書かれています。`follow_scale`(コミット e9cb85d、2026-11-15)はそれより前のもので、その変更には含まれていません。

## How to trigger

コードを読んで確認したもので、実行はしていません。実アプリを(任意の起動方法で。PC にプラグインがあり、保存済みのスキャンがない場合、または `meloet some.meloet`)起動します。Tauri 2.12.5 では、`setup` は `RuntimeRunEvent::Ready` で、設定されたウィンドウを作ったすぐ後に動きます(`tauri-2.12.5/src/app.rs:1441-1445, 2688-2701`)。ページはまだ一つも読みこまれていません。`setup` → `follow_scale` → `core.subscribe` → `Msg::ScreenReady` → `plugins.release()` と `Input::ScreenUp`、という順です。`PluginSide::release` にログ行またはブレークポイントを置けば、ページの `hello` コマンドより前に動くことが分かります。

## Cause

`Core::subscribe`(service.rs:888)は二つのことをします。送り先を追加することと、画面の到着を知らせることです。`follow_scale` の中のシェルの購読は、拡大率の変化だけが必要ですが、到着の知らせまで受け取ります。これが一回目の `subscribe` なので、そこで送られる `ScreenReady` が効いてしまいます(`release` と `screen_up` の印は一度だけ働きます)。

## Suggested fix

最も小さな変更は、シェルが画面への到着を知らせずに購読できるようにすることです。たとえば、`follow_scale` が使う別のコアのメソッドを作ります。それは `hub.subscribe` だけを呼ぶ `Core::listen(messages)`、またはフラグを付けた `subscribe` です。ページの `subscribe` コマンドは、今のまま `ScreenReady` を送ります。別の方法として、`subscribe` ではなく `hello` コマンドから `ScreenReady` を送ることもできます。プロトコル、曲ファイル、文面は変えません。コアの変更(crates/meloet-core/src/service.rs)が必要で、これはこの領域の外にあります。

## Verification

コードを読んで確認しました(実アプリは起動していません)。Tauri 2.12.5(`~/.cargo/registry/src/*/tauri-2.12.5/src/app.rs`。`Cargo.lock` にあるバージョン)では、ビルダーの `setup` クロージャーは `fn setup`(`app.rs:2688-2701`)から呼ばれます。それは、まず設定のウィンドウを作り、それからクロージャーを呼びます。これは `RuntimeRunEvent::Ready`(`app.rs:1442-1443`)の上で、メインスレッドで、イベントループがページの読みこみをまだ一つも行う前に起きます。meloet のクロージャー(`main.rs:557-560`)では、`follow_scale` が `core.subscribe(..)`(`main.rs:254`)を呼びます。`Core::subscribe`(`service.rs:888-891`)は必ず `Msg::ScreenReady` を送り、コアのスレッドは `plugins.release()` と `handle(Input::ScreenUp)` を呼びます(`service.rs:2844-2849`)。ページ自身の `subscribe` コマンド(`main.rs:105`)はページが読みこまれたあとにしか来ません。つまり、シェルの購読は必ず最初になり、スキャン、PC から渡された曲、パックの確認、更新の確認は、ページを待ちません。`SCAN_WAITS_FOR_SCREEN` は一度も効きません。アーキテクチャ 20-10 と CLAUDE.md の CodeLLDB の注記は、スキャンは画面のあいさつのあとに始まると書いています。子プロセスは今も一つずつ(`launch::spawn`)起動されるので、CodeLLDB のハングが戻ったとは主張しません。主に問題になるのは、WebKit の読みこみ中の一時停止と、ウィンドウが遅れることで、それは 20-10 に書かれています。重大度 `medium` は変えません。

訂正: なし。

修正案: `crates/meloet-core/src/service.rs`: 別のメソッド(たとえば `Core::listen(messages)`)で、`self.hub.subscribe(messages, <a telemetry sink that says false>)` だけを呼び、何も送らない。`crates/meloet-app/src/main.rs` の `follow_scale` がこれを使う。ページの `subscribe` コマンドは `Core::subscribe` のまま。テスト: `service/tests.rs` または `plug_tests.rs` で、`wait_for_screen` を使い、`listen` では保留中のスキャンが解放されず、`subscribe` では解放されること。修正のあと、VS Code の「meloet（デバッグ）」での起動でウィンドウが出ることを一度手で確認します。

## Fix

`crates/meloet-core/src/service.rs`: 新しい `Core::listen(messages)` は、`hub.subscribe` を、画面が来ていないと答えるテレメトリーの送り先とともに呼ぶだけで、`Msg::ScreenReady` は送りません。`crates/meloet-app/src/core_handle.rs`(`CoreHandle::listen`)と `crates/meloet-app/src/main.rs` の `follow_scale` がこれを使います。ページの `subscribe` コマンドは `Core::subscribe` のままです。`docs/architecture.md` 20-10 に、シェル自身の購読は数えないと書きました。

テスト: `service::shell_tests::the_shells_own_listening_is_not_the_screens_greeting`(`service/shell_tests.rs`: `wait_for_screen` と、渡された曲がある状態で、`listen` ではスキャンは保留のまま、曲も開かれないこと。`subscribe` ではその両方が始まること)。VS Code の「meloet（デバッグ）」での起動は、修正した人が手で確認していません(アプリを起動していないため)。
