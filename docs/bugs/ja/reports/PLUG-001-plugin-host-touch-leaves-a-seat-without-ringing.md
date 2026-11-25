---
id: PLUG-001
title: プラグインホスト: touch() が席を離してもベルを鳴らさないため、その間に頼まれたブロックでホスト全体が止められることがある
area: PLUG
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# PLUG-001: プラグインホスト: touch() が席を離してもベルを鳴らさないため、その間に頼まれたブロックでホスト全体が止められることがある

## Where

crates/meloet-plughost/src/rt.rs:394-399 (`Handle::touch`: 席を取る。その後に `self.bell.ring()` がない)
crates/meloet-plughost/src/rt.rs:561-575 (`run`: ほかのスレッドが持っている席は飛ばされ、そのあとスレッドは `IDLE_NS` (34 行目) まで、ベルの上で眠る)
crates/meloet-plughost/src/control.rs:241 (`take_hold` は `audio.touch(slot, id, true)` を呼ぶ)
crates/meloet-plughost/src/control.rs:251 (`let_go` は `audio.touch(slot, id, false)` を呼ぶ)
crates/meloet-plugin/src/ipc/ends.rs:228-243 (`await_overdue`: 誰も依頼を取らず `progress` も動かないと、そのスロットとマッピング全体が止められる)

## What happens

プラグインホストのメインスレッドが `Handle::touch` の中にいる (スロットの席を一瞬持っている) とき、エンジンがそのスロットにブロックを頼むと、ブロックのために起きた音声スレッドは席がふさがっているのに気づく。そのため、その席を飛ばし、ほかにすることがないので、ベルで眠り直す。`touch` は席を離すがベルは鳴らさないので、音声スレッドは、100 ms の時間制限 (`IDLE_NS`) が来るか、ほかのスロットが頼まれるまで起こされない。

プラグインが 1 つのホスト (普通の場合。1 つのプロセスにつきプラグインの 1 つのインスタンス) では、ほかに鳴らす者がいない。エンジンの期限は過ぎる (無音または乾いたブロック)。次のブロックの期限で `await_overdue` は、依頼は取られていない、`progress` は動いていない、プラグインの中にいるスレッドはない、と見る。そしてそのスロットと共有ヘッダーを止まったと印を付ける。meloet はプロセスを殺し、そのプラグインを止まったと表示する (「（名前）が止まりました…」)。そのホストのプラグインはすべて、「もう一度読みこむ」まで音が出なくなる。

タイミングに依存し、1 回ごとの発生は少ないが、エンジンがブロックを回している間に、使う人がプラグインの窓でつまみを握る、または離すと、いつでも起こりうる。

## What should happen

メインスレッドは席を離すたびにベルを鳴らす。そうすれば、そこにいる間に頼まれたブロックが飛ばされることはない。これは音声スレッドが頼っている決まりである。`run` のコメント (rt.rs:562-563) は「メインスレッドは席を離すときベルを鳴らす」と書いていて、ほかの席の使い手もみな行う: `install` (rt.rs:347), `remove` (354), `set_param` (366), `params_taken` (375), `outputs` (415), `forget_params` (425)。忙しいホストや遅れたホストを、死んだものと取り違えてはならない (docs/architecture.md 20-2, 23-14)。

## How to trigger

コードを読んで分かったこと。実行はしていない。
1. 窓のある VST3 か LV2 のプラグインを 1 つ読みこむ (そのホストプロセスにプラグインは 1 つ)。エンジンがブロックを回すままにする (再生中、または出力が開いているだけ)。
2. そのプラグインの窓を開き、つまみを動かさずに掴む (VST3 の `beginEdit` / LV2 の `ui:touch` で掴む: `Said::ParamBegin` だけで、ベルを鳴らす `set_param` はない)。または最後の動きの後でつまみを離す (`Said::ParamEnd`)。または、touch のない LV2 のパラメータを 0.5 秒動かさないでおく (`LET_GO`)。
3. エンジンがそのスロットのブロックを、`touch` が席を持っている間に出し、起きた音声スレッドがそのときに席を見ると、2 ブロック後にホストが止められる。

## Cause

`Handle::touch` は、席を取るメインスレッドのメソッドのうち、あとでベル (`self.bell.ring()`) を鳴らさない唯一のものである。音声スレッドは、席を見て回る前にベルの回数を読む (`let seen = bell.count()`)。そのため、その回る前にエンジンが鳴らしたベルでは起きず、あとで鳴らされたベルだけで起きる。鳴らされなければ、スレッドは `IDLE_NS` (100 ms) 眠る。これはエンジンの 2 ブロック分より長く、`await_overdue` の監視は、これを凍ったプロセスと区別できない。

## Suggested fix

兄弟のメソッドと同じように、`Handle::touch` の最後でベルを鳴らす (`Taken` の守りを外したあと):

```rust
pub fn touch(&mut self, slot: u32, id: u32, on: bool) -> Result<(), String> {
    if let Some(run) = self.seat(slot)?.run() {
        run.touch(id, on);
    }
    // A block asked for while the main thread was at the seat was passed over.
    self.bell.ring();
    Ok(())
}
```

プロトコルやファイルの形は変えない。

## Verification

コードを読んで確認した (テストは実行していない。2 つのスレッドとエンジンの間の競合で、順序を強制する小さなテストは見つからなかった)。`Handle::touch` (`rt.rs:394-399`) は、席を取ってあとでベルを鳴らさない唯一のメインスレッドのメソッドである。`install`、`remove`、`set_param`、`params_taken`、`outputs`、`forget_params` はみな、同じコメントとともに鳴らしている。止まったホストへの道は次のとおり。エンジンの `submit` が 1 回鳴らす (`ends.rs:292-296`)。音声スレッドが起き、`seen` を読み、席がふさがっているのを見て、`IDLE_NS` (100 ms) まで眠る。ブロックが `pending` の間、エンジンはもう鳴らさない (`submit` はすぐ戻る)。最初の `wait` が `seen_progress` を記録する。次のブロックで `await_overdue` は、`taken != seq`、`progress` が動いていない (動かすのは `take_request` と `finish` だけ)、プラグインの中にいるスレッドはない、を見て、そのマッピングを止まったと印を付ける。

訂正: 窓は報告の手順が示すより狭い。メインスレッドが `touch` の中で席を持つのは、小さな `Vec` への push の間だけなので、音声スレッドは、その数命令の間に席を見なければならない。1 回の touch に対して発生はまれである。しかし 1 つのプラグインのホストには、それを救うほかのベルがなく、結果はプラグインが止まることなので、重大度 `medium` は変えない (発生頻度から `low` とすることには同意しない)。

修正案: `crates/meloet-plughost/src/rt.rs` の `Handle::touch`: 席のガードを外したあとに `self.bell.ring();` を置き、兄弟のメソッドと同じコメントをつける。競合そのものはテストできない。`touch` でベルの回数が増えることを確かめる単体テストで足りる。

## Fix

- `crates/meloet-plughost/src/rt.rs` の `Handle::touch`: ほかの席の使い手と同じく、席を離したあとにベルを鳴らす。
- テスト: `rt::tests::taking_hold_of_a_parameter_rings_the_bell_after` (meloet-plughost): 音声スレッドが席を見る前に読むようにベルを読む。`touch` (掴む、離す) のあとでベルが動いていなければならない。そうすれば、席がふさがっていると見たスレッドは `IDLE_NS` まで眠らない。競合そのものにはテストはない。`touch` の中にあるメインスレッドの一瞬は、新しい仕組みなしにはテストから止められない。
