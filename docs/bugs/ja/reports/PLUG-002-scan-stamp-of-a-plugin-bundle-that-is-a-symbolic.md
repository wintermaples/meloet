---
id: PLUG-002
title: シンボリックリンクのプラグイン束のスキャン印が変わらないため、リンク先で変わった、または消えたプラグインが二度とスキャンされない
area: PLUG
kind: bug
severity: low
status: fixed
found: 2026-11-25
---

# PLUG-002: シンボリックリンクのプラグイン束のスキャン印が変わらないため、リンク先で変わった、または消えたプラグインが二度とスキャンされない

## Where

crates/meloet-plugin/src/scan.rs:127-155 (`BundleStamp::read` / `add`: 束そのものに `std::fs::symlink_metadata` を使う)
crates/meloet-core/src/plugins/scanner.rs:252-253 (`bundles_in`: 束は名前の終わりで見つかる。リンクも含む)
crates/meloet-core/src/plugins/scanner.rs:269-276 (`places_todo`: 印が変わらなければ記録をそのまま残す)

## What happens

プラグインの置き場所にある束が、それ自体シンボリックリンクだと (たとえば `~/.vst3/Foo.vst3 -> ~/Downloads/Foo/Foo.vst3` や `~/.lv2/foo.lv2 -> /opt/foo/foo.lv2`)、その束は見つかって一度スキャンされますが、その印はリンク自身の更新時刻とリンクの文字列の長さだけになります。リンク先のフォルダに何が起きても、印は同じままです。そのため:

- 使う人がその場でプラグインを更新する (束の中に新しいプラグインが入る、プラグインが消える、別の種類やパラメータの新しいバージョンになる) と、meloet は古い記録を残し続けます。新しいプラグインは棚に届かず、消えたものは棚に残ります。
- リンク先が削除される (リンクがどこも指さなくなる) と、`symlink_metadata` はリンクに対してまだ成功し、`bundles_in` は名前をまだ見つけるので、プラグインは棚に残ります。それを選ぶと読み込みに失敗します。

「もう一度スキャン」は、止まったプラグインに対してしか出ないので、そうでなければ再読み込みされるのは、スキャンのキャッシュを消したときだけです。

## What should happen

docs/architecture.md 20-4: 「Results are remembered by path, modified time and size; later scans look only at changed bundles.」(結果はパス、更新時刻、大きさで覚える。以降のスキャンは変わった束だけを見る)。中身が変わった束は、変わったものと数えなければならない。scan.rs:126 の注記「Links are not followed」は、束を歩くときの束の中のリンクについてのもので、スキャンがリンクを通して実際に読むのはリンク先のフォルダです。

## How to trigger

コードを読んで確かめた内容で、実行はしていません。
1. どこかにプラグインのフォルダを置き、置き場所へリンクする: `ln -s /opt/x/Foo.vst3 ~/.vst3/Foo.vst3`。meloet を起動する。スキャンされる。
2. `/opt/x/Foo.vst3` の中身を、もう一つのクラスを足したバージョンに差し替える (または `/opt/x/Foo.vst3` を削除する)。
3. meloet をもう一度起動する: `BundleStamp::read(~/.vst3/Foo.vst3)` は同じ印を返す (リンクの `symlink_metadata`: ディレクトリではない、大きさはリンクの長さ、リンクの更新時刻)。`places_todo` は `None` を返し、古い記録が残る。

## Cause

`BundleStamp::add` は、最初のパス、つまり束そのものにも `std::fs::symlink_metadata(path)` を呼ぶ。束がディレクトリではないリンクだと、そこで歩くのが止まり、印はリンク自身のものになる。`bundles_in` は、リンクかどうかを見ずに、名前の終わりでエントリを受け取る。

## Suggested fix

束そのものは `std::fs::metadata` で読む (リンクは一番上の一段だけたどる)。中身は `symlink_metadata` のままにする。たとえば `BundleStamp::read` に、一段目用のフラグを渡す。そうすると、どこも指さないリンクはエラーになり、`places_todo` は古い記録を残すので、それを「なくなった」と数えなければならない。そのため `bundles_in` (scanner.rs、別の領域) では、`std::fs::metadata` が失敗するエントリを飛ばす。これで `gone` に入る。プロトコルもファイルの形も変えない。`SCAN_CACHE_VERSION` は変えなくてよい (リンクの印だけが変わり、変化は一度だけ読まれる)。

## Verification

コードを読んで確かめた (テストは実行していない)。`BundleStamp::add` は束のパスそのものに `std::fs::symlink_metadata` を呼ぶ。束がシンボリックリンクだと、それはリンク (ディレクトリではない) なので、印はリンク自身の更新時刻とリンクの文字列の長さになる。`bundles_in` (`scanner.rs:245-256`) は、リンクかどうかに関係なく名前の終わりでエントリを取り、`places_todo` は変わらない印に対して `None` を返す。リンク先の変化は再スキャンされず、どこも指さないリンクは同じ印のまま棚に残る。重大度 `low` のまま (リンクされた束だけの問題で、「もう一度スキャン」は止まらなかったプラグインには出ない)。

訂正: なし。

修正案: `crates/meloet-plugin/src/scan.rs` の `BundleStamp::read`: 束そのものは `std::fs::metadata` で読む (リンクは一番上だけたどる)。その下は `symlink_metadata` のまま。`crates/meloet-core/src/plugins/scanner.rs` の `bundles_in`: `std::fs::metadata` が失敗するエントリ (どこも指さないリンク) を飛ばす。テスト: `scan.rs` (リンクされた束の印は、リンク先の中のファイルが変わると変わる) と、scanner のテスト (どこも指さないリンクは束ではない)。`SCAN_CACHE_VERSION` は変えない。リンクされた束は、変更の後に一度だけ再スキャンされる。

## Fix

`crates/meloet-plugin/src/scan.rs` の `BundleStamp::read`: 束そのものは `std::fs::metadata` で読む (リンクは一番上だけたどる)。その中は前と同じく `symlink_metadata` で読む。`crates/meloet-core/src/plugins/scanner.rs` の `bundles_in`: 束の名前を持ち、どこも指さないエントリは飛ばす。そのため記録は、なくなった束と一緒に消える。`SCAN_CACHE_VERSION` は変えない。

テスト: `scan::tests::a_bundle_that_is_a_link_is_stamped_by_what_it_points_to` (meloet-plugin)、`plugins::scanner::tests::a_bundle_that_is_a_link_is_found_while_it_points_somewhere` (meloet-core)。
