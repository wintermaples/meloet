// The only file that knows about Tauri. Everything else is plain web.
import { invoke } from "@tauri-apps/api/core";

export function report(result: unknown): Promise<void> {
  return invoke("bench_done", { result: JSON.stringify(result) });
}
