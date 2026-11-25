---
id: CORE-STATE-002
title: 名前をつけて保存は、いまの曲自身のフォルダを開くため、新しい曲のフォルダが古い曲のフォルダの中に作られる
area: CORE-STATE
kind: ui-ux
severity: medium
status: fixed
found: 2026-11-25
---

# CORE-STATE-002: 名前をつけて保存は、いまの曲自身のフォルダを開くため、新しい曲のフォルダが古い曲のフォルダの中に作られる

## Where

crates/meloet-core/src/service.rs:3235-3240 (`Effect::PickSaveAs` は `self.start_folder()` を使う)
crates/meloet-core/src/service.rs:3798-3806 (`start_folder`: 曲のファイルの親。つまり曲のフォルダ `<songs>/<name>/` そのもの)

## What happens

以前に保存した曲の場合、「名前をつけて保存…」は、PC の保存の窓を曲のフォルダ `<songs>/<name>/`（`<name>.meloet` と `音/` がある場所）で開きます。使う人が新しい名前を打って保存を押すだけだと、`<songs>/<name>/<new name>/<new name>.meloet` ができます。新しい曲のフォルダが、古い曲のフォルダの中に、その `音/` と並んで置かれるのです。あとで古い曲のフォルダを削除、または移動すると、新しい曲も一緒になくなります。

## What should happen

docs/spec/3-bands.md 89: 「名前をつけて保存」は「選んだ場所に『名前』のフォルダを作る」。docs/architecture.md 14: 曲は `<music>` の中の曲のフォルダ 1 つで、曲のフォルダは 1 曲に 1 つです。窓が最初に開く場所は、曲のフォルダを入れている場所（曲のフォルダを入れているフォルダ、または曲のフォルダの場所）であるべきで、曲のフォルダの中ではいけません。最初の保存は、尋ねずに `<songs>/<name>/` へ行っています。

## How to trigger

コードを読んで確かめました。実行はしていません。
1. 新しい曲を保存します。`<songs>/はじめての曲/はじめての曲.meloet` になります。
2. メニュー「ファイル ▸ 名前をつけて保存…」。`ShellRequest::PickSaveAs { folder: Some(<songs>/はじめての曲) }` が送られ、シェルは `set_current_folder` にそれを渡します（crates/meloet-app/src/pcwindow.rs:124-126）。
3. 「夏の曲」と打って保存を押します。`split_save_as` はそのフォルダを場所として使い、`save_new_with` が `<songs>/はじめての曲/夏の曲/夏の曲.meloet` を作ります。

## Cause

`start_folder` は「ひらく…」と、「保存できない」の窓のために書かれていて（開いている曲のファイルのフォルダを使う）、`Effect::PickSaveAs` がそれを再利用しています。選んだ場所に新しい曲のフォルダを作る保存では、曲のファイルのフォルダは 1 段深すぎます。

## Suggested fix

`Effect::PickSaveAs` は、曲のフォルダの親（`song_path().parent().parent()`）から始め、なければ曲のフォルダを置く場所（songs フォルダ）から始めます。`start_folder` は、開く窓のためにそのまま残します。「保存できない」のフォルダの窓（`Effect::PickFolder`）も、選んだ場所に新しい曲のフォルダを作るので、同じ開始場所が合います。どちらにするかは、リードが決めてよいです。プロトコル、文言、ファイルの形は変えません。

## Verification

コードを読んで確かめました（テストは実行していません）。`Effect::PickSaveAs`（`service.rs:3235-3240`）は、保存の窓を `start_folder()` で開きます。これは保存済みの曲の場合、曲自身のフォルダ `<songs>/<name>/` の親、つまり曲のファイルの親です。`split_save_as`（`state/songs.rs:565-571`）は、窓が返したパスの親を場所として使い、そこに新しい曲のフォルダを作ります。そのため、フォルダを変えずに新しい名前を打つと、`<songs>/<name>/<new name>/` になります。architecture 18（「名前をつけて保存」）と 14（曲のフォルダは songs フォルダの中に 1 曲に 1 つ）、spec 3（「選んだ場所に『名前』のフォルダを作る」）は、窓がどこから始まるかを書いていません。曲のフォルダの中から始めることを意図した記述は、どこにもありません。重要度 `medium` は変えません（コマンドを普通に使うだけで、曲が別の曲のフォルダの中にできるため）。

訂正: なし。

修正案: `crates/meloet-core/src/service.rs` に、選んだ場所に曲のフォルダを作る窓のための 2 つ目の補助関数を置きます。現在の曲のフォルダを入れているフォルダ（`song_path().parent().parent()`）を使い、なければ songs フォルダを使います。どちらも、フォルダである場合だけ使います（フィルタの順番は CORE-STATE-005 を参照）。`Effect::PickSaveAs` がそれを使います。`Effect::PickFolder`（「保存できない」→「場所をえらぶ」。これも選んだ場所に新しい曲のフォルダを作る）もそれを使います。「ひらく…」は `start_folder` のままです。小さな単体テストを、一時フォルダを使って書きます。

## Fix

`crates/meloet-core/src/service.rs`: 2 つ目の補助関数 `Worker::place_folder`（曲のフォルダを入れているフォルダ、なければ songs フォルダ。どちらもフォルダである場合だけ）。共通のフィルタは `first_folder`。`Effect::PickSaveAs` と `Effect::PickFolder`（「保存できない」→「場所をえらぶ」）がそれを使います。「ひらく…」は `start_folder` のままです。`docs/architecture.md` 18 に、窓がどこから始まるかを書きました。

テスト: `service::shell_tests::the_pcs_windows_start_among_the_song_folders_or_in_the_songs_folder`（`service/shell_tests.rs`。コアの新しいテストファイルで、呼ばれた内容を記録するシェルを使う）。
