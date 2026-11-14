# 最初から入れるピアノの音源の候補（調べただけ。入れるのは次の回）

2026-11-14、meloet-research（Sonnet 5.5、effort high）が調べた結果を、棟梁がまとめた物です。
条件: MIT OR Apache-2.0 の配布物に同梱して、形式の変換・間引き・圧縮をして配れること。商用の禁止（NC）は不可。GPL・AGPL・LGPL は不可。

確かさ: A＝一次情報（公式のリポジトリ、配布元の文）、B＝一次情報を引いた二次情報、C＝推論・見積もり。

## 結論

1. **Salamander Grand Piano V3**（Yamaha C5、CC BY 3.0 Unported）が本命。[A]
   - 同梱・変換・間引き・圧縮してよい。条件は帰属表示（作者 Alexander Holm、題名、URI、ライセンスの URI か全文）と、変更したことの表示。[A]
   - 16 ベロシティ、短 3 度ごと（A0〜C8、約 30 音）、リリース音とペダル音あり。SFZ ＋ FLAC（48 kHz / 24 bit で 707 MiB）。[A]
   - 4 段に間引くと、FLAC 16 bit / 44.1 kHz で約 60 MiB、Opus か Vorbis（128 kbps）で約 16 MB の見積もり。[C]
2. **VCSL（Versilian Community Sample Library）のピアノ**（CC0）。クレジットも要らない。[A]
   - グランド 3 台（Kawai 2 種、Steinway B）、アップライト 2 台。段の数（3〜4）と音の数は、ファイル名から読んだだけ。[C]
   - Steinway B を 2 音おき × 3 段で、FLAC 約 76〜150 MB、Vorbis で約 20〜35 MB の見積もり。[C]
3. **FreePats の Upright Piano KW**（CC0）。小さい版は FLAC 2.9 MiB。2 段で家での録音なので、グランドの代わりにはならない。[A]

## 候補の表

| 名前 | ライセンス | 同梱・改変 | 帰属表示 | 楽器と中身 | 大きさ | 確かさ |
|---|---|---|---|---|---|---|
| Salamander Grand Piano V3 | CC BY 3.0 Unported | 可 | 要る | Yamaha C5。16 段、短 3 度ごと、リリース、ペダル | FLAC 707 MiB、WAV 16 bit 394 MiB | A（大きさの見積もりは C） |
| Accurate-Salamander v6.2e | CC BY（3.0 へのリンク） | 可 | 要る（元の作者にも） | Salamander の再調整。641 サンプル、16 段 | FLAC 0.8 GB / バンク | A。2026-11-12 更新で、版が動いている |
| VCSL のピアノ | CC0 | 可 | 不要 | グランド 3、アップライト 2 | Keys 全体で約 680 MB | A（中身の細部は C） |
| Upright Piano KW | CC0 | 可 | 不要 | Kawai アップライト、2 段 | FLAC 32 MiB、小さい版 2.9 MiB | A |
| Headroom Piano | CC BY 4.0 | 可 | 要る | Yamaha C3、5 段 | FLAC 156 MB | A（元の Kontakt 版の権利は未確認） |
| YDP Grand Piano | CC BY 3.0 | 可 | 要る | Disklavier Pro（SF2） | 36 MiB | A（中身の仕様は不足） |
| Splendid Grand Piano | 「Akai が 2000 年ごろにパブリックドメインにした」という主張だけ | 主張どおりなら可 | 不要 | Steinway、4 段 | 256 MB（77 MB という一覧もあり、食いちがい） | B〜C（Akai の一次情報なし） |
| Iowa MIS Piano | 正式なライセンスなし（「制限なしで使ってよい」の文だけ） | 文面どおりなら可 | 不明 | Steinway B、3 段、半音ごと | AIFF 約 1.3 GB | A（許諾の文として弱い） |

## 使えない・条件つき

| 名前 | 理由 | 確かさ |
|---|---|---|
| Maestro Concert Grand | 作者の文に「許可なしに改変して広めるな」 | A |
| Piano in 162 | ライセンスの記載が見つからない | A（「記載なし」を確認） |
| LivingRoom Upright Micro | CC BY-SA 4.0。継承の条件がアプリにかかるかは判断が要る | A（ライセンス）、C（影響） |

## 次の回にすること

- Salamander の FLAC を実際に 4 段だけ取り出し、大きさを測る（上の見積もりは仮定を使っている）。
- 帰属表示を置く場所（About の画面、NOTICE ファイル）を決める。
- VCSL を選ぶなら、clone して段の数・音の数・サンプルレートを実ファイルで確かめる。

## 出どころ（見た日 2026-11-14）

- https://github.com/sfzinstruments/SalamanderGrandPiano（README、LICENSE）[A]
- https://sfzinstruments.github.io/pianos/salamander [A]
- https://archive.org/details/SalamanderGrandPianoV3 [A]
- https://freepats.zenvoid.org/Piano/acoustic-grand-piano.html [A]
- https://www.ir.isas.jaxa.jp/~cyamauch/AccurateSalamander/ [A]
- https://github.com/sgossner/VCSL、https://versilian-studios.com/vcsl-keys/、https://versilian-studios.com/vcsl/ [A]
- https://github.com/sfzinstruments/BengtNilsson.HeadroomPiano [A]
- https://github.com/sfzinstruments/SplendidGrandPiano [B]
- https://people.debian.org/~tg/sf/MuseScore_General_Full.copyright [B]
- https://theremin.music.uiowa.edu/MISpiano.html [A]
- https://github.com/sfzinstruments/MatsHelgesson.MaestroConcertGrandPiano（mcgreadme.txt）[A]
- https://sfzinstruments.github.io/pianos/livingroom_upright_micro_sfz [A]
