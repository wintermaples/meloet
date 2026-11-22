# エフェクトの部品、音の素材の読み手、大きな素材の置き場所

2026-11-18、meloet-research（Sonnet 5.5）が調べた結果を、棟梁がまとめた物です。
確かさ: A＝一次情報（公式の文書・リポジトリ・ライセンスの文）、B＝一次情報を引いた二次情報、C＝推論・個人の投稿。2 年より前の版の情報は 1 段下げてある。

## 結論

1. **エフェクトは自分で書く**（C。理由は A の資料から）。借りられる部品は、どれも「パラメータをなめらかに変える」「デノーマルの守り」「音のスレッドで allocate しない」のどれかが足りず、足す手間が書く手間とほぼ同じ。設計は公開の物を使う:
   - EQ: W3C Note「Audio EQ Cookbook」（RBJ の式）か、Andrew Simper の SVF（cytomic.com、公開の知識と序文にある）。[A]
   - コンプレッサー: Giannoulis, Massberg, Reiss「Digital Dynamic Range Compressor Design — A Tutorial and Analysis」JAES 60(6), 2012（書誌だけ確かめた）。[A]
   - リバーブ: Freeverb（作者 Jezar が 2000 年にパブリックドメインにした、とクレートと Debian の写しが引く。作者の元の文は見られなかった）[B]、Dattorro 1997（JAES 45(9)。PDF は画像で中身は読めず、特許は調べていない）[A（存在のみ）]。
   - 使えない: `synfx-dsp` は GPL-3.0。nih-plug の `plugins/` の一部、augmented-audio の一部は GPLv3・AGPLv3（examples と MIT の所だけなら可）。[A]
   - 参考: `fundsp`（MIT OR Apache-2.0、0.23.0）はリバーブ・リミッター・コーラスを持つが、コンプレッサーなし、デノーマルの守りなし、`Limiter::new` は allocate する。`freeverb`（MIT、irh）は tick で allocate しないが、なめらかさとデノーマルの守りなし。`biquad`（MIT OR Apache-2.0）は `no_std`。[A]
   - デノーマルは、音のスレッドの入口で FTZ/DAZ（x86 の MXCSR、ARM の FPCR）を立てる手がある（`no_denormals`、MIT）。cpal のコールバックで効くかは確かめていない。[C]
2. **素材の読み手は `symphonia`（FLAC と Vorbis だけ）**。純粋な Rust、0.6.1（2026-08-13）、FLAC は FFmpeg の 0.51〜0.74 倍の時間（2023 の BENCHMARKS.md）。MPL-2.0 だが、書きかえずに依存するだけなら meloet のコードは MIT/Apache のまま（Mozilla の MPL FAQ Q8・Q11・Q13）。配るときは MPL のソースの場所を知らせる（「meloet について」の 2 段目に書く）。[A]
   - `claxon`（FLAC、Apache-2.0、0.4.3 は 2020 年）と `lewton`（Vorbis、MIT OR Apache-2.0、0.10.2 は 2021 年）も使えるが、どちらも動いていない。[A→B]
   - Opus は symphonia 0.6.1 では読めない（作りかけ）。読むなら libopus（C）が要る。→ Opus は使わない。[A]
3. **置き場所**:
   - Git LFS（GitHub、無料）: 保管 10 GiB、転送量 10 GiB/月。clone・fork・pull・zip のダウンロードは、どれも**持ち主の**転送量に入る。超えるとその月は LFS が止まる。500 MB の素材なら 20 回の clone で月の分がなくなる計算。公開の個人のリポジトリには向かない。[A]
   - GitHub Releases の添付: 1 ファイル 2 GiB 未満、1 リリース 1,000 ファイルまで、合計と転送量の上限は書かれていない。ただし利用規約 9 節に「ほかとくらべて、かなり多すぎる転送」は制限しうる、とある（数はない）。[A]
   - ふつうのリポジトリ: 100 MB のファイルは push を拒む、リポジトリは 1 GB が理想。[A]
   - `build.rs` でネットにつなぐと、docs.rs・Nix・Debian のようにネットなしで作る所でこける。ダウンロードは別の道具（`scripts/` や xtask）にし、sha256 で確かめ、取った物は使いまわすのがよく使われる作法。[A（docs.rs）・C（作法）]
   - archive.org などの配布元から毎回のビルドで取るのは、配布元に迷惑（Internet Archive のブログ 2023-05-29 は「大きく取るときは、ゆっくり、前に連絡」）。ライセンスを確かめた版を自分の Releases に置きなおすほうが、礼儀としても、こわれにくさでも、よい。[A・C]

## 出どころ（見た日 2026-11-18）

| URL | 中身 | 確かさ |
|---|---|---|
| https://crates.io/api/v1/crates/（fundsp、biquad、synfx-dsp、valib、freeverb、symphonia、claxon、lewton、rubato、ogg、opus、no_denormals ほか） | 版、ライセンス、日付 | A |
| https://github.com/SamiPerttu/fundsp | 中身、allocate、デノーマル | A（要約を通して） |
| https://github.com/irh/freeverb-rs、https://docs.rs/crate/freeverb/latest/source/src/lib.rs | MIT、パブリックドメインの記述 | A |
| https://github.com/pdeljanov/Symphonia（BENCHMARKS.md、symphonia-codec-opus） | MPL-2.0、速さ、Opus は作りかけ | A |
| https://www.mozilla.org/en-US/MPL/2.0/FAQ/ | MPL の扱い | A |
| https://www.w3.org/TR/audio-eq-cookbook/ | EQ の式 | A |
| https://cytomic.com/technical-papers/ | SVF | A |
| https://ccrma.stanford.edu/~dattorro/EffectDesignPart1.pdf | Dattorro 1997（存在のみ） | A |
| https://www.aes.org/e-lib/browse.cfm?elib=16354 | Giannoulis らの論文の書誌 | A |
| https://ccrma.stanford.edu/~jos/pasp/Freeverb.html | Freeverb の形 | B |
| https://docs.github.com/en/billing/concepts/product-billing/git-lfs | LFS の無料の枠と超えたとき | A |
| https://docs.github.com/en/repositories/working-with-files/managing-large-files/about-storage-and-bandwidth-usage | fork・pull は親の転送量 | A |
| https://docs.github.com/en/repositories/releasing-projects-on-github/about-releases | Releases の上限 | A |
| https://docs.github.com/en/site-policy/acceptable-use-policies/github-acceptable-use-policies | 帯域の使いすぎ | A |
| https://docs.rs/about/builds | docs.rs はネットなし | A |
| https://blog.archive.org/2023/05/29/let-us-serve-you-but-dont-bring-us-down/ | 大きく取るときの作法 | A |

## わからなかったこと

- LFS の 1 ファイルの上限は、同じページに 2 GB と 5 GB の両方が書いてある（食いちがい）。
- archive.org の規約の本文、Freeverb の作者の元の README、Dattorro の特許。
- FTZ/DAZ が cpal（PipeWire）のコールバックのスレッドで効くか → 試験で確かめる。
