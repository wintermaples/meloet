---
id: CORE-PLAY-C-002
title: AIの状態の書きこみで、バイト列を受け取ったあとにプラグインのプロセスが止まると、曲の状態ではなく書きこんだ状態で読みなおされる
area: CORE-PLAY-C
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# CORE-PLAY-C-002: AIの状態の書きこみで、バイト列を受け取ったあとにプラグインのプロセスが止まると、曲の状態ではなく書きこんだ状態で読みなおされる

## Where

crates/meloet-core/src/plugins/hosts.rs:656-666 (`Ask::SetState`: `Reply::Ok` のとき、書きこんだ状態が `Slot::state` になる。`reload` が使う状態)
crates/meloet-core/src/service.rs:3574-3591 (`PluginAsk::PutState`: `set_state` のあとに `state`。二つ目が `Gone` なら `StatePut(Err(Gone))` と伝える)
crates/meloet-core/src/play/plugs/aistate.rs:186-190 (`Err(HostError::Gone)` → `revive(ticket, restored)`。`from_song` は取り消しかやりなおしのあとだけ)
crates/meloet-core/src/play/plugs/aistate.rs:351-366 (`reload_at` → `Effect::ReloadPlugin`。`Hosts` が覚えている状態で読みこむ)
crates/meloet-core/src/play/plugs/aistate.rs:199-203 (AIに渡す文面)

## What happens

AIのリンクが、音が鳴っている間に状態をプラグインへ書きこむと (`set_plugin_state`、`set_plugin_state_text`、`load_preset`、Surge XT のツール)、プラグインのホストは、プラグインが状態を本当に受け取る前に `Ok` を返します。docs/architecture.md 24-16 G によると、Surge XT は次のオーディオのブロックでようやく受け取り、ホストは続く `GetState` を最大3ブロック待たせます。その間にプロセスが止まると (オーディオのブロックで状態を読むときに、その状態がプラグインをクラッシュさせる。あるいは固まって殺される)、コアは `StatePut(Err(Gone))` を受け取り、`Effect::ReloadPlugin` でプラグインを読みなおします。このとき `Hosts` は、覚えている状態、つまりいま AI が書きこんだ状態で読みこみます。

AIには「曲の状態で読みなおしました。手は作られていません」と伝わります。曲は古い状態のままですが、プラグインは AI の状態で鳴っています。その後の保存では、プラグインに状態を問い合わせるため、AI の状態が手を作らずにファイルへ書かれます。その状態がプラグインをクラッシュさせたものなら、読みなおしでもまたクラッシュします (`plugin_unreadable` で `Stopped` のまま)。すると、使う人が押すたびの「もう一度読みこむ」で同じ状態が読みこまれ、またクラッシュします。このプラグインは削除するしかなくなります。

## What should happen

docs/architecture.md 24-16 G: 「プロセスが止まる (`Gone`) → `ReloadPlugin` を、`Hosts` が保持している状態で行う (失敗した書きこみは状態を置き換えない)」。また `AiPutEnd::Stopped` (ai.rs:716-718): 「プラグインのプロセスは、受け取っている途中で止まった (状態が壊れているかもしれない)。曲の状態で読みなおす」。`Stopped` で終わったあと、曲とプラグインは曲の状態になっているべきです (ai.rs:704-705: 「ほかの処理のあと、曲とプラグインは曲の状態になっている」)。

## How to trigger

コードを読んで確かめたもので、実行はしていません。
1. Surge XT (VST3) のトラックが鳴っている (少なくとも出口が動いていて、ブロックが走っている)。
2. AIのリンクから `set_plugin_state` を、コンポーネントの部分が壊れたコンテナで呼ぶ (architecture 24-16 G: 「コンポーネントの部分を半分に切った Surge XT の VST3 の状態は、プラグインのホストをクラッシュさせる (SIGSEGV)」。音が鳴っていると、プラグインは `SetState` が `Ok` を返したあと、次のブロックでそれを読む)。
3. `Hosts` がバイト列を `Slot::state` として保存する。続く `GetState` は `Gone` を受け取る。コアは `Effect::ReloadPlugin` で読みなおす。`Hosts::reload` が、壊れたバイト列をもう一度書きこむ。
同じことは、クラッシュする状態でなくても起こります。SetState が `Ok` を返してから、読み出しの答えが来るまでの間にプロセスが止まると (5秒後に殺される固まり) 、プラグインは AI の状態で読みなおされ、曲は自分の状態のままになります。

## Cause

`Run::answer` は `Ask::SetState` について、プロセスが答えた時点で状態を覚えます。ブロックが走っているプラグインでは、その答えは「バイト列を受け取った」という意味しかありません。`ai_state_put` は `Err(Gone)` を「書きこみが失敗した」と扱い、取り消しかやりなおしが間に入っていない限り (`restored`)、`Hosts` の覚えている状態を使う `ReloadPlugin` で復元します。

## Suggested fix

`ai_state_put` で `Err(HostError::Gone)` のときは、書きこんだプラグインを必ず曲から復元します: `self.revive(ticket, true, effects)` (`from_song` の経路、`load_from_song` は取り消しの場合に既にあります)。同じプロセスにあるほかのプラグインで、復元されるものは `Hosts` の覚えている状態を使い続けます。別案として、`Hosts` は書きこんだ状態を、そのスロットの後続の `GetState` が答えてから初めて覚えることもできます。プロトコル、ワイヤ、ファイルの形は変えません。

## Verification

コードを読んで確かめたもので、テストは実行していません (実際のプラグインと音を鳴らす状態が必要で、それは起動していません)。`Run::answer` の `Ask::SetState` (`hosts.rs:656-666`) は、ホストのプロセスが `Ok` を返すとすぐ、書きこんだバイト列を `Slot::state` として保存します。`PluginAsk::PutState` (`service.rs:3574-3591`) は `set_state` を送り、それから `state` を送ります。一つ目が `Ok` で二つ目が `Gone` のとき、コアは `StatePut(Err(Gone))` を受け取ります。`ai_state_put` (`aistate.rs:186-190`) は `revive(ticket, restored)` を呼びます。`from_song` が真になるのは取り消しかやりなおしのあとだけです。それ以外では `reload_at` が `Effect::ReloadPlugin` を積み、`Run::reload` が `slot.state` を読みこみます。その値はいま書きこんだ状態になっています。architecture 24-16 G は、ブロックが走っている間に書きこんだ状態を Surge XT が受け取るのは次のブロックで、`SetState` の答えのあとだと書いています。よって、そこでのクラッシュは `Ok` のあとに起きます。同じ章は「`Hosts` の保持している状態で `ReloadPlugin` する (失敗した書きこみは状態を置き換えない)」と約束しており、答えの文面は「曲の状態で読みなおした」と言っています。24-16 G で測った場合 (半分に切った状態で「読みなおされ、曲は無事」) は、音を鳴らしていない場合で、書きこみそのものが失敗します。その場合は今も正しく動きます。重大度 `medium` は変えません (曲のプラグインが、曲が持っていない状態で鳴り、クラッシュする状態は読みなおすたびに書きこまれるため)。

訂正: なし。

修正計画: `crates/meloet-core/src/play/plugs/aistate.rs` の `ai_state_put`: `Err(HostError::Gone)` のとき `self.revive(ticket, true, effects)` を呼び、書きこんだプラグインを曲から (`load_from_song` で) 読みなおします。同じプロセスのほかのプラグインは `ReloadPlugin` の経路のままです。aistate のテスト: 答えが `Gone` の書きこみは、`ReloadPlugin` ではなく、曲の状態を持った `Effect::LoadPlugin` になる。`crates/meloet-ai/tests/plugstate.rs` (無視されるテスト、実際のプラグインを使う) に半分に切った場合があれば、音を鳴らして動かせる人がいるときに同じものを足します。

## Fix

決めたこと (上の修正計画ではなく、リードの判断): `Hosts` は、AIの書きこみのあとにプラグインが自分の状態を読み返すまで、読みなおしのために以前の状態を保持します。`Hosts::try_state` (`crates/meloet-core/src/plugins/hosts.rs`) は、状態を覚えずに書きこみます。その後の `Hosts::state` が、プラグインの言う状態を覚えます。その間に止まると、書きこみの前の状態で読みなおします。`PluginAsk::PutState` (`service.rs`) がこれを使います。取り消しとやりなおしの書き戻し (`Hosts::set_state`) は、従来どおりすぐに覚えます。書きこんだプラグインの戻り道は `ReloadPlugin` のままです。そのため、AI には読みなおされる内容を次のように伝えるようにしました: 「書きこむ前にあった状態で (取り消しかやりなおしが間に入っていた場合は曲の状態で) 読みなおす」 (`play/plugs/aistate.rs`、`crates/meloet-ai/src/tools/plugstate.rs`、`ai.rs` の `AiPutEnd::Stopped` の説明、docs/architecture.md 24-16 G)。

テスト: `tests/plugin_state_put.rs` の `a_reload_is_at_the_rate_asked_for_and_without_a_state_tried_and_never_read_back` (プロセスの代わりのホストが、渡された内容を書き残す。`try_state` のあと止まると、読みなおしは書きこむ前の状態を受け取る。`set_state` のあとは書きこんだ状態)。`play::plugs::aistate::tests::a_state_that_stops_the_process_makes_no_step_and_the_plugin_is_loaded_again` は文面を確かめます。
