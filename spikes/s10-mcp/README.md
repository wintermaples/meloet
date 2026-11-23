# s10-mcp — talking MCP to a running meloet

A spike for round 10, "the AI link": an AI app starts a small program `meloet-mcp` as a stdio
MCP server, and that program connects to the running meloet through a socket inside the PC.
Not product code. Do not copy it; take what was learned.

It has its own `[workspace]` and `Cargo.lock`, so the product workspace does not change.
Tried on 2026-11-23 with rustc 1.100.0, rmcp 3.11.0 (pinned), tokio 1.54.1, schemars 1.2.3,
Claude Code 2.1.330.

| Part | File | What |
|---|---|---|
| fake meloet | `src/lib.rs` | two tools on a pretend song: `song_overview` (read only), `add_notes` (writes) |
| (a) stdio server | `src/main.rs` | `FakeMeloet::new().serve(stdio())` |
| (b) client | `client.py` | standard library only; `legacy` and `new` handshakes, over `--cmd` or `--socket` |
| (c) meloet side | `src/bin/app.rs` | the same tools on `$XDG_RUNTIME_DIR/s10-mcp/ai.sock` |
| (d) bridge | `src/bin/bridge.rs` | the `meloet-mcp` role: std only (no tokio), lines between stdio and the socket |
| (e) licenses | `deny.toml` | a copy of the product's |

```bash
cargo build
python3 -I client.py legacy --cmd target/debug/s10-mcp
python3 -I client.py new    --cmd target/debug/s10-mcp
target/debug/app &                                   # then:
python3 -I client.py new --cmd target/debug/bridge   # client -> bridge -> socket -> app
python3 -I client.py legacy --cmd target/debug/bridge --off error   # with app stopped
cargo deny check licenses
```

## What we learned

### Handshake (rmcp 3.11 server)

- `serve(stdio())` answers **both** handshakes with no setting:
  - old: `initialize` → `notifications/initialized` → `tools/list` → `tools/call`. It echoes the
    version the client asked for (`2025-11-25`, `2025-06-18` were tried).
  - new `2026-07-28`: `server/discover` with `_meta` (`io.modelcontextprotocol/protocolVersion`,
    `.../clientCapabilities`; `.../clientInfo` is optional) → `tools/list` → `tools/call`, each with
    the same `_meta`. Results carry `resultType: "complete"`; `tools/list` adds `ttlMs: 0` and
    `cacheScope: "public"`; discover lists `supportedVersions` 2024-11-05 … 2026-07-28 and puts
    `serverInfo` under `_meta`.
- How (source `rmcp-3.11.0/src/service/server.rs`, `serve_server_with_ct_inner`): before a
  lifecycle is chosen, a `server/discover` is answered and the server keeps waiting; an
  `initialize` picks the old lifecycle; any other request with complete `_meta` picks the new one.
  A discover without `_meta` gets `-32602` and an `initialize` after it still works (the fallback).
- `ClientLifecycleMode` (`Initialize` / `Discover` / `Auto`, `src/service/client.rs`) is for the
  **client** side only. The server needs nothing.
- On `2026-07-28` the server may not send requests to the client (sampling, elicitation): rmcp
  returns an error for them (`server.rs`, "server-to-client requests are not allowed").

### Tools

- `#[tool_router]` on the impl, `#[tool(annotations(title, read_only_hint, destructive_hint,
  idempotent_hint, open_world_hint))]` on each method, `#[tool_handler(router = self.tool_router)]`
  on `impl ServerHandler`, and `get_info()` returning
  `ServerConfig::new(ServerCapabilities::builder().enable_tools().build())`.
  Doc comments become descriptions. Arguments are `Parameters<T>` with `T: JsonSchema +
  Deserialize`; the schema is JSON Schema 2020-12 with `$defs`.
- The annotations arrive as `readOnlyHint`, `openWorldHint`, etc. exactly as set.
- `u8` becomes `minimum: 0, maximum: 255`. The real tools need `#[schemars(range(...))]` (or
  their own checks, which they need anyway).
- A failed run is `CallToolResult::error(...)` → `isError: true` with text the model can act on.

### The socket (meloet side)

- Directory `$XDG_RUNTIME_DIR/<name>` made with `DirBuilder::mode(0o700)`; if it exists, check
  it is a directory we own (`symlink_metadata`, uid) and narrow it to 0700.
- Old socket: `std::os::unix::net::UnixStream::connect`. `ConnectionRefused` → nobody answers →
  remove it; `Ok` → another meloet is running → stop; `NotFound` → fine. Tried with an app
  killed by SIGKILL: the next start printed "removing stale socket" and went on.
- `umask(0o177)` around `bind`, then restore: the socket came out `srw-------`, the directory
  `drwx------`. umask is per process; in meloet do it where no other thread is making files, or
  rely on the 0700 directory and `chmod` afterwards.
- Peer uid: tokio `UnixStream::peer_cred()` (uses `getsockopt(SO_PEERCRED)` on Linux,
  `tokio-1.54.1/src/net/unix/ucred.rs`) gives uid, gid and pid. **std's `peer_cred` is still
  unstable** on rustc 1.100 (`peer_credentials_unix_socket`, E0658). The reject path was tried by
  pretending to want uid 0 (`app --expect-uid 0`): the connection was closed before any MCP
  (the client saw a reset). A real other user was not tried (needs sudo; the 0700 directory
  already stops them).
- rmcp's `transport-async-rw` takes the `tokio::net::UnixStream` as is: `service.serve(stream)`.
  One `FakeMeloet` clone per connection; the state is shared.
- On SIGTERM the app removes the socket. Open connections end when the runtime stops.

### The bridge (`meloet-mcp`)

- Plain std works: one thread copies socket → stdout, the main thread reads stdin lines and
  writes them to the socket. `serde_json` is only needed to answer by itself when meloet is away.
  When stdin closes the process exits 0 (the copy thread does not keep it alive).
- When meloet is away, three ways were tried (`--off`):
  - `exit`: message on stderr, exit 1. The client sees the pipe close before any answer.
  - `error`: a JSON-RPC error with the message for every request, including `initialize` /
    `server/discover`. The AI app treats the server as failed to start.
  - `stub` (best): answers `initialize` itself (with the message in `instructions`), lists one
    read-only tool `meloet_status`, and every `tools/call` returns `isError: true` with
    "meloet で『AI とつなぐ』をオンにしてください…". `server/discover` gets `-32601` so a
    `2026-07-28` client falls back to `initialize`. On each later request the bridge tries the
    socket again; when meloet appears it greets it with the saved `initialize` (drops the
    answer), sends `notifications/initialized`, emits `notifications/tools/list_changed` to the
    AI app, and from then on just copies. When meloet goes away it goes back to the stub.
    All of this was tried in one session (stub → meloet starts → real tools → meloet stops → stub).
  - Why `stub` is better: the model can read the message and tell the person what to do, and the
    person does not have to restart the AI app after turning the link on. With `exit`/`error` the
    message only reaches logs. (How Claude Code shows `exit`/`error` was not tried; see below.)

### Claude Code

- `claude -p "<prompt>" --strict-mcp-config --mcp-config <file> --allowedTools mcp__meloet__song_overview`
  loads the server for that run only and does not touch `~/.claude.json`. (Put the prompt
  before `--allowedTools`, which takes several values.)
- One run, bridge → socket → app: **connected and called the tool**; the answer was printed as is.
- Claude Code 2.1.330 used the **new `2026-07-28` handshake**: `server/discover` (id
  `server-discover-probe-1`) then `tools/list` and `tools/call`, each with full `_meta`. Client
  capabilities: `roots.listChanged`, `elicitation` (form, url), extension
  `io.modelcontextprotocol/tasks`. `tools/call` also sent `_meta.progressToken` and
  `claudecode/toolUseId`.
- It **started the server twice** (two bridge processes, two socket connections, each with
  discover + `tools/list`); the call went through the second. meloet must accept several
  connections and must not treat a connection as "the AI".
- For the owner, to try it for real: `claude mcp add --transport stdio --scope user meloet -- /usr/bin/meloet-mcp`
  (this writes `~/.claude.json`; not run here).

### Licenses (features `server`, `macros`, `schemars`, `transport-io`, `transport-async-rw`)

`cargo deny check licenses` with the product's `deny.toml`: **licenses ok**. 83 crates. No GPL,
AGPL or MPL. rmcp, rmcp-macros and unicode-general-category are Apache-2.0 only (fits
MIT OR Apache-2.0; rmcp's LICENSE notes some old parts are still MIT); the rest MIT and/or
Apache-2.0, plus Unicode-3.0, Zlib, Unlicense in OR or AND with MIT/Apache. rmcp brings chrono,
uuid, sha2, futures, tracing, schemars 1, pastey and darling; tokio 1.54.1, serde_json, chrono
are already in the product lock at the same versions.

## A shape for the product (proposal)

- `crates/meloet-mcp`: the bridge binary. std (+ `serde_json`) only, no rmcp, no tokio. Stub mode
  as above. Finds the socket by `$XDG_RUNTIME_DIR`.
- meloet side: a new crate (say `meloet-ai`) with rmcp + tokio, owning the listener, the peer-uid
  check and the tool handlers. It runs on its own thread with a current-thread tokio runtime,
  started and stopped by the setting. Tools turn into the same intents the UI sends to
  `meloet-core` (so each write is one undo step, and the UI sees the change through the normal
  rev diffs). It knows nothing of Tauri.
- Tools (first list): read only — `song_overview`, `list_tracks`, `get_notes` (track, range in
  ticks), `get_tempo_map`, `transport_state`, `levels` (numbers, not audio); writing — `add_notes`,
  `edit_notes` / `delete_notes` (by take id and index), `create_track`, `set_tempo`,
  `add_marker`, `undo`. All with `openWorldHint: false`; read tools `readOnlyHint: true`.
