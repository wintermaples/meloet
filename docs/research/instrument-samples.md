# 最初から入れる音色の素材（第 4 回）

2026-11-18、meloet-research（Sonnet 5.5）が調べた結果を、棟梁がまとめた物です。前の調べ物は [`piano-samples.md`](piano-samples.md)（2026-11-14）。
条件: MIT・Apache-2.0 の配布物に同梱して、変換・間引き・圧縮して配れる。CC0・CC BY はよい。NC・ND・GPL・LGPL・AGPL は不可。CC BY-SA は使う前にヤチヨさんに聞く。生の楽器を計算で作ってそれらしくするのはなし（シンセの音色とエレクトロキットは計算で作ってよい）。
確かさ: A＝一次情報（配布元・リポジトリ・ライセンスの文・規格）、B＝一次情報を引いた二次情報、C＝推論・個人の投稿。

## 鍵盤（ピアノ ・ 鍵盤のグループ）

### グランドピアノ: Salamander Grand Piano V3（CC BY 3.0 Unported）[A]

- 2026-11-14 から変わりなし。作者 Alexander Holm、元の配布は https://archive.org/details/SalamanderGrandPianoV3 （ライセンス URL http://creativecommons.org/licenses/by/3.0/）。SFZ の組みなおしは kinwie（https://github.com/sfzinstruments/SalamanderGrandPiano 、最後のコミット 2022-01-03）。README は「この sfz から作ったと書きそえてほしい」と頼んでいる。
- 中身（GitHub 版の SFZ を読んだ）: 30 音（A0〜C8、短 3 度ごと、1 音が 3 鍵）× 16 段 = 480。ベロシティの段の境は不ぞろい（v1 1–26、v2 27–34、v3 35–36、v4 37–43、v5 44–46、v6 47–50、v7 51–56、v8 57–64、v9 65–72、v10 73–80、v11 81–88、v12 89–96、v13 97–104、v14 105–112、v15 113–120、v16 121–127）。リリース音（ハンマーの音）88 個、弦の共鳴 69 個、ペダルの音 4 個。合わせて 641。
- 配布の形: FreePats の SFZ+FLAC 48 kHz/24 bit（707 MiB、ハッシュの公開なし）、archive.org の tar.bz2（48 kHz/24 bit 1,448,107,335 B、md5 44c5be2c774aeb968cf9bc09952511dd・sha1 90dfa11fa1f812a1cd956f38e66bc82820c918aa。44.1 kHz/16 bit 488,713,261 B。作者の Ogg Vorbis 版 78,010,482 B）。
- 帰属表示に要る物（CC BY 3.0 の 4(a)(b)・3(b)）: 作者の名前、題名、作者が指した URI、ライセンスの URI か写し、手を加えたことの表示。間引き・圧縮・形式の変換は「手を加えた」に入る。

### エレクトリックピアノ: Greg Sullivan E-Pianos の Wurlitzer EP200（CC BY）[A]

- 作者 Greg Sullivan（http://www.sullivang.net/ ）、SFZ の組みなおし kinwie（https://github.com/sfzinstruments/GregSullivan.E-Pianos 、2020-08-23）。
- ライセンスの出どころが 2 つ: リポジトリの README と LICENSE は CC BY 3.0、作者のサイトのトップは「ここの楽器はみな CC BY 4.0」（2026-10-21 に変えたと書いてある）。どちらも CC BY。**食いちがいあり**。作者本人の新しいほう（4.0）を採り、サイトの文の写しを日付つきで残すのがよい。
- Wurlitzer EP200: A1〜C7 の 64 鍵を 42 ファイル（2〜9 鍵ごと）、4 段、FLAC 合わせて約 2.3 MB。録音は 1999〜2004 年ごろで古い。作者自身が「調律がよくない」「ループの雑音」と書いている。聞いて確かめる。
- ほかの 2 台（同じパック）: Yamaha CP80（全域、4 段、44 MB。電気グランドでローズとは別の音）、Hohner Pianet T（2 段、強弱の幅がせまい）。
- 使えない: jRhodes3c・3d（Rhodes Mark I）は CC BY-NC（作者の README と LICENSE）。第三者のまとめが CC0 と書くが、作者の文を採る。VCSL の TX81Z「FM Piano」（CC0）はハードの FM シンセの録音で、本物の電気ピアノではない。

### オルガン: 実機のトーンホイール（ハモンドのような）の素材は見つからなかった

- FreePats の Electric Organ・Pipe Organ（CC0）は、ソフトシンセ（setBfree、Aeolus）の録音で、計算で作った音なので対象外。[A]
- 実機を録った CC0: **VCSL Pipe Organ**（Simon Dalzell 録音、Loud と Quiet の 2 段、C1〜C6 の短 3 度ごと 21 音、1 段で約 46 MB）と VCSL Renaissance Organ（2 半音ごと、段なし）。どちらも教会や昔のオルガンの音で、ポップスの「オルガン」とは雰囲気がちがうかもしれない（聞いていない）。[A（中身）・C（音の印象）]
- Pianobook の Hammond L102、Polyphone の Hammond B3、HamOrg などは、再配布できるかが読めない。[A（記載なし）・C]

## ドラム、ベース、ストリングス

（調べ物が返ったら足す）

## 出どころ（見た日 2026-11-18）

| URL | 中身 | 確かさ |
|---|---|---|
| https://github.com/sfzinstruments/SalamanderGrandPiano | README、LICENSE、SFZ、ファイル | A |
| https://archive.org/details/SalamanderGrandPianoV3（/metadata/） | 元の配布、ハッシュ、ライセンス URL | A |
| https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html | FLAC 707 MiB ほか | A |
| https://creativecommons.org/licenses/by/3.0/legalcode | 帰属表示の条件 | A |
| https://github.com/sfzinstruments/GregSullivan.E-Pianos | 3 台、CC BY 3.0 | A |
| http://www.sullivang.net/ | CC BY 4.0（2026-10-21） | A |
| https://sfzinstruments.github.io/pianos/jrhodes3d | jRhodes は CC BY-NC | A |
| https://github.com/smpldsnds | jRhodes を CC0 と書く第三者のまとめ（食いちがい） | C |
| https://github.com/sgossner/VCSL、https://versilian-studios.com/vcsl/ | CC0、Pipe Organ、Renaissance Organ | A |
| https://freepats.zenvoid.org/Organ/electric-organ.html ほか | ソフトシンセの録音 | A |
| https://www.pianobook.co.uk/packs/hammond-l102 | ライセンスの記載なし | A |
