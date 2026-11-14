// 書体を読みこむ入口。main.ts から一度だけ import する。
// woff2 は @fontsource のものを手を加えずに使う（OFL-1.1）。
import "@fontsource/ibm-plex-sans-jp/400.css";
import "@fontsource/ibm-plex-sans-jp/500.css";
import "@fontsource/ibm-plex-sans-jp/600.css";
import "@fontsource/ibm-plex-mono/400.css";
import "@fontsource/ibm-plex-mono/500.css";

export { fontFamily } from "./tokens";
