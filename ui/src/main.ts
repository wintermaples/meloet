import { mount } from "svelte";
import "./design/fonts";
import "./components/common.css";
import { pageLanguage } from "./bridge";
import { writeCssVariables } from "./design/tokens";
import { useLanguage } from "./text";

// The language is decided before the first component is made: the words of the page do not change until it loads again.
const language = await pageLanguage();
useLanguage(language);
document.documentElement.lang = language;
// The first picture, before the state comes, is also drawn with all the value variables in place.
writeCssVariables(100);
const { default: App } = await import("./App.svelte");
mount(App, { target: document.getElementById("app")! });
