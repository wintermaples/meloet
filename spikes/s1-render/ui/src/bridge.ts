// The only file that knows about Tauri. Everything else is plain web.
import { Channel, invoke } from "@tauri-apps/api/core";

export function report(result: unknown): Promise<void> {
  return invoke("bench_done", { result: JSON.stringify(result) });
}

export function startTicks(hz: number, secs: number, onTick: () => void): Promise<void> {
  const ch = new Channel<ArrayBuffer>(() => onTick());
  return invoke("ticks", { hz, secs, channel: ch });
}
