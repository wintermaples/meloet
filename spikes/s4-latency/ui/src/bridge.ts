// The only file that knows about Tauri.
import { Channel, convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

export type Msg = { json?: any; bin?: ArrayBuffer };
export type Transport = "event" | "chan-json" | "chan-bin" | "pull";

export const rustClock = () => invoke<number>("clock");
export const devices = () => invoke<any>("devices");
export const devicesNow = () => invoke<any[]>("devices_now");
export const cpuSample = () => invoke<any>("cpu_sample");
export const finish = (r: unknown) => invoke("finish", { result: JSON.stringify(r) });
export const onRust = (name: string, f: (p: any) => void) => listen<any>(name, (e) => f(e.payload));

export async function connect(t: Transport, hz: number, onMsg: (m: Msg) => void): Promise<() => Promise<ArrayBuffer>> {
  const ch = new Channel<any>((m) => (m instanceof ArrayBuffer ? onMsg({ bin: m }) : onMsg({ json: typeof m === "string" ? JSON.parse(m) : m })));
  if (t === "event") {
    await listen<any>("key", (e) => onMsg({ json: e.payload }));
    await listen<any>("frame", (e) => onMsg({ json: e.payload }));
  }
  await invoke("connect", { mode: t, hz, channel: ch });
  const url = convertFileSrc("frame", "meloet");
  return () => fetch(url).then((r) => r.arrayBuffer());
}
