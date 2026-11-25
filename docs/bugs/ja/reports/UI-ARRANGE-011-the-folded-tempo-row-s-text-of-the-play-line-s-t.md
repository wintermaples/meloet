---
id: UI-ARRANGE-011
title: 折りたたんだテンポの行にある、再生の線のテンポと拍子の文字が、曲の頭と一緒にスクロールして消える
area: UI-ARRANGE
kind: ui-ux
severity: low
status: fixed
found: 2026-11-25
---

# UI-ARRANGE-011: 折りたたんだテンポの行にある、再生の線のテンポと拍子の文字が、曲の頭と一緒にスクロールして消える

## Where

ui/src/components/timeline/TempoBand.svelte:362 (`<span class="m-num m-lbl now" style:left="{-view.scrollX}px" …>{now}</span>`)
ui/src/components/Arrange.svelte:1096-1119 (`now`: 再生の線にあるテンポと拍子。再生中は一秒に最大 10 回書き換える)

## What happens

「テンポ ・ 拍子の変更」の行を折りたたむと、その時間の側には「96.00 ・ 4/4」が出る。これは再生の線の位置にあるテンポと拍子である (再生中、Arrange が `showNow` で更新する)。文字は `left: -view.scrollX` に置かれる。つまり曲の頭の位置で、横にスクロールするたびに左へ動き、アレンジを幅 (約 80 px) より多くスクロールすると、もう見えなくなる。再生中に表示が再生の線に従うと、最初のページがめくれたあとで見えなくなる。ちょうど値が変わるときに。

## What should happen

7-readings (round 6 のテンポの帯の表): 折りたたんだ行の「96.00 ・ 4/4」は、「再生の線の値」を示す。見本 F6-AutoRest (この行がある他の見本、たとえば F-History も) は、行の `left: 4px` に置いている。曲の位置の印としてではなく、行の見出しとして。再生の線に従う値には、くっつく曲の位置がない。

## How to trigger

実行せず、コードを読んで確かめた。帯を折りたたんだ状態で、アレンジを横へ数小節スクロールする (Shift + ホイール)、または再生して最初のページを過ぎる。行の「96.00 ・ 4/4」の文字がなくなる。

## Cause

文字は、行の左端ではなく、曲の頭 (`-view.scrollX`) を基準に置かれている。

## Suggested fix

行の左端に置く (`style:left` を外し、`.now` の `margin-left: space.1` は残す)。見本のとおり。WebKit の再描画が小さいままか確かめる (`tools/measure/webkit-damage.sh`。文字は一秒に最大 10 回変わる)。プロトコルや文言の変更はなし。

## Verification

テストを走らせず、読んで確かめた。`TempoBand.svelte:362` は、折りたたんだ行の文字を `left: -view.scrollX` (曲の頭) と `margin-left: space.1` で置く。`now` は再生の線にある値で、`Arrange.svelte` の `showNow` が更新する。7-readings A-20: 「「96.00 ・ 4/4」 on the folded row | The values at the play line. | spec 3」。再生の線に従う値は行の見出しであり、曲の頭に置くと、自分の幅より多く横にスクロールしたあとは見えなくなる。見本は左端に置いている (見本はスクロールしていないので、二つを区別できない。上の読み方がそれを示す)。この件のテストは一つだけ (`ui/tests/round6.timeline.test.ts:270`)。文字は確かめるが、置く位置は確かめていない。

修正案 (提案どおり): `ui/src/components/timeline/TempoBand.svelte` で `.now` の span から `style:left` を外す (`position: absolute; top; margin-left` は残すので、行の左端に来る)。そのあと、曲を再生し、行を折りたたんだ状態で `tools/measure/webkit-damage.sh` を一度走らせる。文字はアレンジがめくれる間、その場で変わるようになるため。

## Fix

コミットはまだ (lead がコミットする)。

- `ui/src/components/timeline/TempoBand.svelte`: 折りたたんだ行の「96.00 ・ 4/4」は `style:left` を持たなくなった。行の左端に置かれる (`.now` は `margin-left: space.1` を保つ)。`tools/measure/webkit-damage.sh` は走らせていない (実際のアプリが要る)。

テスト: `ui/tests/round6.timeline.test.ts` — 「the folded row's tempo and time signature at the play line stay at the row's left edge when the arrange is scrolled」。
