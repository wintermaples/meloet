# FluidR3_GM（と、そこから作られた SoundFont）を同梱できるか

2026-11-18、meloet-research（Sonnet 5.5）が調べた結果を、棟梁がまとめた物です。きっかけ: この PC の `/usr/share/sounds/sf2/default-GM.sf2` は `FluidR3_GM.sf2`（Debian の `fluid-soundfont-gm`）で、音色が豊富なので、MIT なら最初から入っている音色の素材の候補にしたい（ヤチヨさん 2026-11-18）。
確かさ: A＝一次情報、B＝一次情報を引いた二次情報（Debian の copyright もここ）、C＝推論・個人の投稿。

## 結論

1. **ライセンスは MIT**（A）。作者 Frank Wen の README「I hereby release Fluid under the MIT license, as described in COPYING.」、COPYING は標準の MIT（Copyright (c) 2000-2002, 2008 Frank Wen）、SF2 の中の INFO にも「Licensed under the MIT License.」。同梱するときは著作権の表示と MIT の文を付ける。
2. **録音の出どころは確かでない**（B〜C）。作者は「公開されていた（パブリックドメインの）サンプルを直した物と、主に自分の録音、それと協力者」としか書かず、音色ごとの出どころの一覧はない。2008 年に Debian の main に入れるとき「ライセンスの食いちがい」と「すべてのサンプルの著作権の証拠」が話題になり、作者が README に協力者と出どころの文を足して通った。商用の素材がまざっているという指摘は、見つからなかった（ないとは言い切れない）。古い R3 の README（Public Domain ・ All Rights Reserved と書いてあるという写し）とも食いちがう（C、全文は取れず）。
3. **オルガン**: Drawbar Organ（ハモンド風）は音名のないサンプルが 2 個だけで、実機を音ごとに録った形ではない（C、ファイルの中の名前から）。Rock Organ（17 個ほど）と Church Organ（21 個ほど）は音ごとに録った形だが、実機かどうかは一次情報で決められない。実機のトーンホイールの録音は、FluidR3 にも見つからなかった。
4. **派生**: MuseScore General（S. Christian Collins、MIT ＋ 一部 CC0 ・ PD、最後の配布 0.2 は 2020-07-10）は FluidR3Mono の派生。ピアノ・FM エレピ・シンセベースなどを入れかえたが、オルガン・ベース・ふつうのドラムは FluidR3 のまま（変更履歴、A）。オルガンの出どころは FluidR3 と同じなので、使うなら FluidR3_GM のほうが表示が少なく、元がはっきりする。
5. **ほかの GM の SoundFont**: GeneralUser GS は独自の文（音楽づくり ・ ソフトの中で使ってよい、直してよい。再配布 ・ 変換して配ってよいとは書いていない。作者自身が「出どころは 100 % 確かではない」）→ 人が決める。TimGM6mb は GPL-2（不可）。Arachno、SGM は再配布の許可なし ・ 商用素材の疑い（不可）。

## 出どころ（見た日 2026-11-18）

| URL | 中身 | 確かさ |
|---|---|---|
| https://sources.debian.org/data/main/f/fluid-soundfont/3.1-6/COPYING | MIT の全文 | A |
| https://sources.debian.org/data/main/f/fluid-soundfont/3.1-6/README | MIT の宣言、素材の文、協力者（この PC の README と同じ） | A |
| `/usr/share/sounds/sf2/FluidR3_GM.sf2`（この PC） | INFO の MIT の文、プリセットとサンプルの名前 | A |
| https://linux.debian.devel.mentors.narkive.com/sRgZRAqZ/rfs-fluid-soundfont-fluid-r3-general-midi-soundfont | 2008 年の Debian のやりとり | B |
| https://ftp.osuosl.org/pub/musescore/soundfont/MuseScore_General/ （Readme、License、Sample_Sources.csv、Changelog） | MuseScore General の出どころと変更 | A（2020 年） |
| https://github.com/musescore/MuseScore/blob/master/share/sound/FluidR3Mono_License.md、FluidR3MonoChangeLog.txt | FluidR3Mono の MIT と変更 | A |
| https://raw.githubusercontent.com/mrbumpy409/GeneralUser-GS/main/documentation/LICENSE.txt | GeneralUser GS の文（要約で） | A |
| https://metadata.ftp-master.debian.org/changelogs/main/t/timgm6mb-soundfont/stable_copyright | TimGM6mb は GPL-2 | B |
| https://www.scribd.com/doc/293685272/Fluid-R3-Readme | 古い README（全文は取れず） | C |
