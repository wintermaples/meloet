---
id: CORE-STATE-005
title: 曲のフォルダがなくなると、PC のファイルの窓が曲のフォルダではなく、どこでもない所から始まる
area: CORE-STATE
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# CORE-STATE-005: 曲のフォルダがなくなると、PC のファイルの窓が曲のフォルダではなく、どこでもない所から始まる

## Where

crates/meloet-core/src/service.rs:3798-3806 (`start_folder`)

## What happens

曲にファイルはあるのに、そのフォルダがもうない場合 (USB メモリを抜いた、フォルダを ファイルの管理ツールで動かしたか消した)、`start_folder` は `None` を返します。そのため「ひらく…」、「名前をつけて保存…」、および「保存できない」のフォルダの窓は、GTK が自分で選ぶ場所で開き、曲のフォルダでは開きません。この場面はまさに「保存できない」が出る時です (ファイルへの上書き保存が、フォルダがないため失敗した)。使う人は、曲のフォルダを一人で探すことになります。

## What should happen

その関数自身のドキュメントコメント: 「使っている曲のファイルのフォルダ。なければ曲のフォルダ」。曲のフォルダがなくなった場合は、曲のフォルダ (`<music>`、docs/architecture.md 14) を使うべきです。

## How to trigger

コードを読んで確かめたもので、実行はしていません。
1. 曲を保存する。その後、そのフォルダ `<songs>/<name>/` を meloet の外で消す (または USB メモリに保存して、それを抜く)。
2. Mod S を押す。`SaveTo::File` が失敗し、「保存できない」が出る。
3. 「場所をえらぶ」: `Effect::PickFolder` → `start_folder()` が `None` → `ShellRequest::PickFolder { folder: None }`。そのため、シェルは `set_current_folder` を呼ばない (crates/meloet-app/src/pcwindow.rs:124-126)。

## Cause

`.or_else(|| self.songs_dir.clone())` が `.filter(|d| d.is_dir())` より前にあります。曲にパスがあるだけで曲のフォルダが選ばれ、それがフィルターで捨てられます。その後に戻る先がありません。

## Suggested fix

戻る前に、曲のフォルダをフィルターにかける: `self.model.song_path().and_then(|p| p.parent()).filter(|d| d.is_dir()).map(PathBuf::from).or_else(|| self.songs_dir.clone().filter(|d| d.is_dir()))`。プロトコル、文面、ファイルの形は変えません。

## Verification

コードを読んで確かめたもので、テストは実行していません。`start_folder` (`service.rs:3798-3806`): `song_path().and_then(parent).map(PathBuf::from).or_else(|| songs_dir.clone()).filter(|d| d.is_dir())`。曲にパスがあると `or_else` は使われず、なくなったフォルダはフィルターで捨てられます。結果は `None` で、その関数自身のコメントが約束する曲のフォルダにはなりません。重大度 `low` は変えません。

訂正: なし。

修正計画: `crates/meloet-core/src/service.rs` の `start_folder`: 戻る前に、それぞれの候補をフィルターにかける (`…parent().filter(|d| d.is_dir()).map(PathBuf::from).or_else(|| self.songs_dir.clone().filter(|d| d.is_dir()))`)。同じ形の二つ目のヘルパーを加える CORE-STATE-002 と一緒に行う。

## Fix

`crates/meloet-core/src/service.rs` の `start_folder`: それぞれの候補は、戻る前にフィルターにかけられます (`first_folder`: 曲のフォルダがディレクトリならそれ、そうでなければ曲のフォルダがディレクトリなら曲のフォルダ)。CORE-STATE-002 と一緒に行いました (`place_folder` も同じフィルターを使う)。

テスト: `service::shell_tests::the_pcs_windows_start_among_the_song_folders_or_in_the_songs_folder` (曲のフォルダが消えた後の部分: 「ひらく…」、「名前をつけて保存…」、「保存できない」→「場所をえらぶ」は、曲のフォルダで始まる)。
