---
id: UI-ROLL-002
title: ノートの一覧と CC の一覧: 並びが変わる値の後に Tab を押すと、次の欄が別のノートで開いて編集される
area: UI-ROLL
kind: bug
severity: medium
status: fixed
found: 2026-11-25
---

# UI-ROLL-002: ノートの一覧と CC の一覧: 並びが変わる値の後に Tab を押すと、次の欄が別のノートで開いて編集される

## Where

- `ui/src/components/pianoroll/NoteListPanel.svelte:149-164` — `finish` は決めた値を送り、そのあと次の欄を行 `e.at + next.row` で `open()` で開く。その `draft` は答えの前に表示されていたテイクから読まれる。
- `ui/src/components/pianoroll/NoteListPanel.svelte:118-123` — 入力の枠は行番号 (`editing.at`) で覚えられている。`editSpec` は、新しいテイクでその行にあるノートから作り直される。
- `ui/src/components/pianoroll/NoteListPanel.svelte:153` — 命令は、そのとき表示されているテイクの `indexes[e.at]` に送られる。
- `ui/src/components/pianoroll/noteFields.ts:315-317` — `commitCommand` は入力された値を、いまその行にあるノートの `spec.current` と比べる。そのため、変えていない枠でも変更として送られる。
- `ui/src/components/pianoroll/CcListPanel.svelte:102-106`, `:132-146` と `ui/src/components/pianoroll/ccFields.ts:176-182` — 「CC の一覧」でも同じ。枠はテイク内の番号 (index) で覚えられている。

## What happens

コードを読んで分かったこと。実行はしていない。ノートの一覧には、行 0 に C3 (1.1.0000)、行 1 に D3 (1.2.0000)、行 2 に E3 (1.3.0000) がある。行 0 の「位置」を開いて `1.4` と入力し、Tab を押す。
1. `finish` が index 0 に `setNotesField` (start) を送り、すぐに行 0 の「音」を古いテイクの文字 `C3` で開く。
2. Rust は開始順の新しいテイク (D3, E3, C3) で答える。行 0 は今 D3 になり (位置・長さ・ベロシティは行に表示される)、動かした C3 は行 2 で選ばれる。行 0 で開いている枠にはまだ `C3` が入っている。
3. 入力せずに Enter か Tab を押すと、新しいテイクの index 0 に対して `setNotesField` key = 48 が送られ、D3 が C3 になる。動かしたノートの音名を新しく入力すると、代わりに D3 が変わる。

Shift Tab でも同じことが起きる。同じ開始位置のノートの間で「音」を変えたときも同じ (順番は開始位置、次に音の高さ)。

「CC の一覧」では、サステインの点が 2 つあり、index 0 は 1.1 (127)、index 1 は 1.2 (0) である。index 0 の位置を後ろの値に変えて Tab を押すと、「値」の枠が index 0 で開く。答えのあとの index 0 は別の点で、同じ行に `127` が入っている。Enter を押すと、その点に `moveControls` の値 +127 が送られ、ペダルが上がらないままになる。

## What should happen

仕様書 3「ノートの一覧」: 「Tab で右の欄へ進む」。つまり、同じノートの右にある欄へ進む。変更していない枠は何も変えないこと (そのとき `commitCommand` は null を返す想定)。アーキテクチャ 16: ノートはテイクの id と index で指す。前のテイクから取った index で、別のノートを編集してはいけない。

## How to trigger

上に書いたとおり。コードから読んだもので、実行はしていない。「位置」を入力して、ほかのノートの開始位置を越えるように動かす。または、同じ開始位置のノートの中で「音」を入力する。そのあと Tab か Shift Tab を押し、Enter を押す。

## Cause

枠は行番号 (ノートの一覧) かテイク内の index (CC の一覧) を覚えていて、その `draft` は答えの前のテイクから埋められる。答えのテイクが届くと、`editSpec` はその行や index にいま来ているノートや点を追うが、枠の文字は変わらない。そのため、決めた操作は別の場所にいるノートを対象にしてしまう。パネルの冒頭コメント「入力中の行は、一覧の行番号で覚えている (決めるとテイクの id が変わる)」は、新しい id のことは書いているが、新しい並びのことは書いていない。

## Suggested fix

行ではなく、編集したノートを追うこと。Rust は編集のあとで編集したノートを選ぶ (アーキテクチャ 16) ので、ノートの一覧は、新しいテイクが表示されたら、選ばれたノートの行で次の欄を開けばよい。その欄の文字は新しいテイクから取る。CC の一覧も同様。または、もっと簡単な方法として、ノートや点を動かせる値 (位置、音。CC の一覧では位置) を入れたあとは、Tab で次へ進まず、枠を閉じる。プロトコルは変えず、新しい文言も入れない。

## Verification

コードを読んで確認した (実行はしていない)。`finish` (`NoteListPanel.svelte:149-164`) は値を送り、すぐに同じ行番号の次の欄を、答えの前に表示されていたテイクの `draft` で開く。`editSpec` (123) は、新しいテイクでその行にあるノートから作り直される。`commitCommand` (`noteFields.ts:315-317`) は入力された数字を、その別のノートの `spec.current` と比べるため、変えていない古い文字が、別のノートへの変更として新しいテイクの id で送られる (Rust はそれを受け付ける。id は表示中のテイクのもの)。テイクの id が変わっても、枠は閉じも再入力もされない (冒頭コメントは、新しい id をまたいで行番号を保つのを意図的としている)。`CcListPanel.svelte:132-146` も index で同じことをする。例の内容は正しい。モデルはノートを開始位置の順、次に音の高さの順に保つ。

修正案 (報告の 2 つのうち小さいほう。新しい仕組みは作らない): `ui/src/components/pianoroll/NoteListPanel.svelte` と `CcListPanel.svelte` で、決めた値が実際に送られ (`command !== null`)、その欄が行を動かせる (ノートの一覧: `position`, `key`、CC の一覧: 位置) とき、古いテイクから次の欄を開かない。答え (`send()` は rev を返す) を待ち、新しい id のテイクが表示されるのを待ってから、Rust が選んだノートの行 (アーキテクチャ 16: 編集したノートは編集後に選ばれる。`pr.selectedNotes` を `indexes` で行にする) で次の欄を開き、その文字は新しいテイクから取る。その行が見つからなければ枠を閉じる。並びを変えられない欄 (長さ、ベロシティ、CC の値) は今の動きのまま。テスト: 報告の例を両方の一覧で確かめる (並びを変える位置の後に Tab、そのあと Enter: 2 つ目の命令は送られない。枠は動かしたノートの上にある)。

## Fix

プロトコルは変えない。

- `ui/src/components/pianoroll/NoteListPanel.svelte`: Tab (または Shift Tab) で実際に送られた「位置」か「音」を決めたあと、枠を閉じ、次の欄は答えの rev と Rust が今表示するテイクを待つ (`after`)。そのあと、Rust が選んだノート (編集したノート) の行で、そのテイクの文字を使って開く。Rust が同じテイクのままなら、今までと同じ行で開く。行が見つからなければ何も開かない。ほかの欄は今まで通り、すぐに次へ進む。
- `ui/src/components/pianoroll/CcListPanel.svelte`: CC の一覧の「位置」も同様 (動かした点が選ばれた点)。
- テスト: `ui/tests/pianoroll.panels.test.ts` "the next field opens on the moved note once Rust's take is shown, with its text. Deciding it unchanged sends nothing" と "when Rust changed nothing (the same take), the next field opens on the same row"。`ui/tests/pianoroll.r5-notes.test.ts` "Tab after a position that changes the order opens 「値」 of the moved point once Rust's take is shown; unchanged it sends nothing (UI-ROLL-002)"。正しい動きに合わせて変えたもの: `pianoroll.panels.test.ts` の "↑ ↓ by 1 …" は、音の高さを変えたあとの Shift Tab で答えを待つようになった。
