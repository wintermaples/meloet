#!/usr/bin/env python3
"""A small MCP client (standard library only), to try the AI link by hand.

  python3 -I tools/ai/mcp_client.py [--new] [--cmd PROGRAM | --socket PATH] [TOOL [JSON]] ...

--cmd    the bridge to start (default: target/release/meloet-mcp); it finds meloet by
         $XDG_RUNTIME_DIR, as an AI app's copy would
--socket talk to meloet's socket itself ($XDG_RUNTIME_DIR/meloet/ai.sock)
--new    the 2026-07-28 handshake (server/discover, `_meta` on every request); else
         initialize and notifications/initialized
TOOL     a tool to call after tools/list, with its arguments as JSON (default {});
         several may follow one another

Prints every line sent (>>) and received (<<). Exits 1 when a call says isError.
"""

import json
import os
import socket
import subprocess
import sys

NEW = "2026-07-28"
OLD = "2025-11-25"
CLIENT = {"name": "meloet-tools-client", "version": "0"}


class Lines:
    def __init__(self, cmd, sock):
        self.proc = None
        if sock:
            self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            self.sock.connect(sock)
            self.w = self.sock.makefile("wb")
            self.r = self.sock.makefile("rb")
        else:
            self.proc = subprocess.Popen(cmd, stdin=subprocess.PIPE, stdout=subprocess.PIPE)
            self.w, self.r = self.proc.stdin, self.proc.stdout
        self.next = 1

    def send(self, msg):
        data = json.dumps(msg, ensure_ascii=False)
        print(">>", data, flush=True)
        self.w.write(data.encode() + b"\n")
        self.w.flush()

    def request(self, method, params, meta):
        params = dict(params)
        if meta:
            params["_meta"] = dict(meta, **params.get("_meta", {}))
        msg = {"jsonrpc": "2.0", "id": self.next, "method": method, "params": params}
        self.next += 1
        self.send(msg)
        while True:
            line = self.r.readline()
            if not line:
                print("<< (closed)", flush=True)
                return None
            print("<<", line.decode().rstrip(), flush=True)
            reply = json.loads(line)
            if reply.get("id") == msg["id"]:
                return reply

    def close(self):
        self.w.close()
        if self.proc:
            self.proc.wait(timeout=10)


def main(argv):
    new, cmd, sock, calls = False, None, None, []
    args = iter(argv)
    for a in args:
        if a == "--new":
            new = True
        elif a == "--cmd":
            cmd = [next(args)]
        elif a == "--socket":
            sock = next(args)
        elif a.startswith("-"):
            sys.exit(__doc__)
        elif a.lstrip().startswith("{") and calls:
            calls[-1][1] = a
        else:
            calls.append([a, "{}"])
    if not cmd and not sock:
        root = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..")
        cmd = [os.path.normpath(os.path.join(root, "target", "release", "meloet-mcp"))]

    link = Lines(cmd, sock)
    meta = None
    if new:
        meta = {
            "io.modelcontextprotocol/protocolVersion": NEW,
            "io.modelcontextprotocol/clientCapabilities": {},
            "io.modelcontextprotocol/clientInfo": CLIENT,
        }
        link.request("server/discover", {}, meta)
    else:
        link.request(
            "initialize",
            {"protocolVersion": OLD, "capabilities": {}, "clientInfo": CLIENT},
            None,
        )
        link.send({"jsonrpc": "2.0", "method": "notifications/initialized"})
    link.request("tools/list", {}, meta)
    failed = False
    for name, arguments in calls:
        reply = link.request(
            "tools/call", {"name": name, "arguments": json.loads(arguments)}, meta
        )
        failed |= reply is None or bool((reply.get("result") or {}).get("isError"))
    link.close()
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
