import { mount } from "svelte";
import "./design/fonts";
import "./components/common.css";
import App from "./App.svelte";
import { writeCssVariables } from "./design/tokens";

// The first picture, before the state comes, is also drawn with all the value variables in place.
writeCssVariables(100);
mount(App, { target: document.getElementById("app")! });
