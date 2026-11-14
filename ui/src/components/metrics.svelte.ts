// いまの拡大率と devicePixelRatio での数値（CSS px）。WebGL の印（glSlot）に渡す数など、
// CSS の変数ではなく JS の数が要る所で使う。
import { SCALES, tokensAt, type Scale } from "../design/tokens";
import { current } from "../state/app.svelte";

let dpr = $state(typeof window === "undefined" ? 1 : window.devicePixelRatio || 1);

/** devicePixelRatio が変わったら（窓を別の画面へ動かした、など）知らせる。やめる関数を返す。 */
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
  /** 名前（"space.1"、"mock.arrange.bar" など）の、いまの CSS px。 */
  px(name: string): number {
    const key = `${this.scale}@${dpr}`;
    let table = cache.get(key);
    if (!table) cache.set(key, (table = tokensAt(this.scale, dpr)));
    const v = table[name];
    if (typeof v !== "number") throw new Error(`数値の名前がない: ${name}`);
    return v;
  },
  /** Rust が送る 100 % の px に率をかける。 */
  scaled(px100: number): number {
    return (px100 * this.scale) / 100;
  },
};
