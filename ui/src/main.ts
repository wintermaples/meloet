import { mount } from "svelte";
import "./design/fonts";
import "./components/common.css";
import App from "./App.svelte";
import { writeCssVariables } from "./design/tokens";

// 状態が届く前の最初の絵も、数値の変数がそろった形で描く。
writeCssVariables(100);
mount(App, { target: document.getElementById("app")! });
