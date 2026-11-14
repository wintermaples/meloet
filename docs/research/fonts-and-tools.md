# 書体と道具のライセンス

2026-11-14、meloet-research（Sonnet 5.5、effort high）が調べた結果を、棟梁がまとめた物です。確かさ: A＝一次情報、B＝二次情報、C＝推論。

## IBM Plex Sans JP と IBM Plex Mono

- ライセンスは SIL Open Font License 1.1。MIT OR Apache-2.0 のアプリに同梱してよい。アプリ本体のライセンスは変わらない（OFL FAQ 1.3、1.4）。[A]
- 守ること: 著作権表示と OFL の本文を同梱する（条件 2）。書体だけを単独で売らない（条件 1）。[A]
- IBM の LICENSE.txt は「Reserved Font Name "Plex"」を指定している。一方、npm の @fontsource の LICENSE にはその文がない。**食いちがい**があるので、安全側（予約名あり）で扱う。[A]
  - meloet は @fontsource の woff2 を**そのまま**同梱する（自分でサブセットを作りなおすと「変更版」になり、名前に Plex を使えなくなる）。[A]（OFL FAQ 2.2、2.6、5.9）
  - 配る形を作るとき（第 8 回）に、IBM の LICENSE.txt をそのまま同梱する。
- @fontsource/ibm-plex-sans-jp 5.3.1、@fontsource/ibm-plex-mono 5.3.0。license 欄はどちらも `OFL-1.1`。woff2 は unicode-range で分けた細切れ（Sans JP は 1 つの太さで 123 切れ、約 1.4〜1.5 MB）。[A]
- Sans JP が JIS X 0208 を全部ふくむかは、一次情報で確かめられなかった。[不明]

## 画面のテストと型の検査の道具

| パッケージ | 版 | ライセンス | 確かさ |
|---|---|---|---|
| vitest | 5.1.0 | MIT | A |
| svelte-check | 4.7.8 | MIT | A |
| @sveltejs/vite-plugin-svelte | 7.3.1 | MIT（vite 8 だけを受ける） | A |
| jsdom | 31.0.1 | MIT（Node 22.22.2 以上） | A |

## ts-rs

- 12.0.1、MIT。Rust 2024 edition のクレートから使える。serde の `rename_all`・`tag`・`content` などを読む（既定の `serde-compat`）。[A]
- MSRV の表記が 1.78.0 と 1.88.0 で食いちがう。meloet は Rust 1.100 なので、どちらでも問題ない。[A]

## 出どころ

- https://github.com/IBM/plex/blob/master/LICENSE.txt、https://openfontlicense.org/ofl-faq/ [A]
- https://registry.npmjs.org/@fontsource/ibm-plex-sans-jp/latest、https://registry.npmjs.org/@fontsource/ibm-plex-mono/latest [A]
- https://registry.npmjs.org/（vitest、svelte-check、@sveltejs/vite-plugin-svelte、jsdom）[A]
- https://docs.rs/ts-rs/12.0.1/ts_rs/、https://crates.io/api/v1/crates/ts-rs [A]
