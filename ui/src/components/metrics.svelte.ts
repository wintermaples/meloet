// Values (CSS px) at the current scale and devicePixelRatio. For places that need a JS number instead of a CSS variable,
// such as the numbers passed to the WebGL mark (glSlot).
import { SCALES, tokensAt, type Scale } from "../design/tokens";
import { current } from "../state/app.svelte";

let dpr = $state(typeof window === "undefined" ? 1 : window.devicePixelRatio || 1);

/** Reports when devicePixelRatio changes (the window moved to another screen, for example). Returns a function that stops it. */
export function watchDevicePixelRatio(): () => void {
  let query: MediaQueryList | null = null;
  const listen = () => {
    query?.removeEventListener("change", onChange);
    query = window.matchMedia?.(`(resolution: ${window.devicePixelRatio}dppx)`) ?? null;
    query?.addEventListener("change", onChange);
  };
  const onChange = () => {
    dpr = window.devicePixelRatio || 1;
    listen();
  };
  listen();
  return () => query?.removeEventListener("change", onChange);
}

const cache = new Map<string, Record<string, number | string>>();

export const metrics = {
  get scale(): Scale {
    const s = current.state?.workspace.scale ?? 100;
    return (SCALES as readonly number[]).includes(s) ? (s as Scale) : 100;
  },
  get dpr(): number {
    return dpr;
  },
  /** The current CSS px of a name ("space.1", "mock.arrange.bar", ...). */
  px(name: string): number {
    const key = `${this.scale}@${dpr}`;
    let table = cache.get(key);
    if (!table) cache.set(key, (table = tokensAt(this.scale, dpr)));
    const v = table[name];
    if (typeof v !== "number") throw new Error(`no value with this name: ${name}`);
    return v;
  },
  /** Multiplies px at 100 % sent by Rust by the scale. */
  scaled(px100: number): number {
    return (px100 * this.scale) / 100;
  },
};
