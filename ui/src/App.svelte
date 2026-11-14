<script lang="ts">
  // A stand-in screen that only shows the bridge is connected. The real screen replaces it.
  import { onMount } from "svelte";
  import { clockOffsetUs, connect, send, type AppState } from "./bridge";

  let app = $state<AppState | null>(null);
  let rev = $state(0);
  let telemetryCount = $state(0);
  let lastSeq = $state<number | null>(null);
  let offsetUs = $state<number | null>(null);
  let failure = $state<string | null>(null);

  onMount(() => {
    connect({
      onState(next, nextRev) {
        app = next;
        rev = nextRev;
      },
      onTelemetry(t) {
        telemetryCount += 1;
        lastSeq = t.seq;
      },
      onError: (e) => (failure = String(e)),
    })
      .then(async () => {
        offsetUs = await clockOffsetUs();
        await send({ type: "setViewport", width: window.innerWidth, height: window.innerHeight });
      })
      .catch((e) => (failure = String(e)));
  });
</script>

<main>
  {#if failure}
    <p data-test="failure">{failure}</p>
  {:else if app}
    <p data-test="song">{app.song.name} — {app.song.tempoBpm} BPM — {app.song.meterNum}/{app.song.meterDen}</p>
    <p data-test="rev">rev {rev} / viewport {app.workspace.viewportWidth}×{app.workspace.viewportHeight} / scale {app.workspace.scale}%</p>
    <p data-test="tracks">{app.song.tracks.map((t) => `${t.number} ${t.name} (${t.instrument}, ${t.gainDb} dB)`).join(", ")}</p>
    <p data-test="lower">lower: {app.workspace.layout.lower.map((b) => `${b.band} ${b.height}`).join(", ") || "-"}</p>
    <p data-test="status">status: {app.status.message?.id ?? "-"}</p>
    <p data-test="telemetry">telemetry: {telemetryCount} (seq {lastSeq ?? "-"}) / clock offset {offsetUs === null ? "-" : Math.round(offsetUs)} µs</p>
    <button onclick={() => send({ type: "toggleBand", band: "pianoRoll" })}>toggleBand pianoRoll</button>
    <button onclick={() => send({ type: "testTone", output: "" })}>testTone</button>
  {:else}
    <p data-test="connecting">connecting…</p>
  {/if}
</main>
