#!/usr/bin/env python3
"""(b) A tiny MCP client, standard library only.

  python3 -I client.py legacy|new  --cmd PROGRAM [ARGS...]
  python3 -I client.py legacy|new  --socket PATH

legacy: initialize -> notifications/initialized -> tools/list -> tools/call
new:    server/discover -> tools/list -> tools/call, each request with the
        2026-07-28 _meta keys and no initialize at all.
Prints every line sent (>>) and received (<<).
"""

import json
import socket
import subprocess
import sys

NEW = "2026-07-28"
OLD = "2025-11-25"
CLIENT = {"name": "s10-client", "version": "0.0.0"}


class Line:
    def __init__(self, args):
        if args[0] == "--cmd":
            self.proc = subprocess.Popen(args[1:], stdin=subprocess.PIPE, stdout=subprocess.PIPE)
            self.w, self.r = self.proc.stdin, self.proc.stdout
        elif args[0] == "--socket":
            self.proc = None
            self.sock = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            self.sock.connect(args[1])
            self.w = self.sock.makefile("wb")
            self.r = self.sock.makefile("rb")
        else:
            sys.exit("need --cmd or --socket")

    def send(self, msg):
        data = json.dumps(msg, ensure_ascii=False)
        print(">>", data, flush=True)
        self.w.write(data.encode() + b"\n")
        self.w.flush()

    def recv(self):
        line = self.r.readline()
        if not line:
            print("<< (closed)", flush=True)
            return None
        print("<<", line.decode().rstrip(), flush=True)
        return json.loads(line)

    def call(self, msg):
        self.send(msg)
        while True:
            reply = self.recv()
            if reply is None or reply.get("id") == msg["id"]:
                return reply

    def close(self):
        self.w.close()
        if self.proc:
            self.proc.wait(timeout=5)
            print("exit code:", self.proc.returncode, flush=True)


def meta():
    return {
        "io.modelcontextprotocol/protocolVersion": NEW,
        "io.modelcontextprotocol/clientInfo": CLIENT,
        "io.modelcontextprotocol/clientCapabilities": {},
    }


def tool_calls(line, params_extra):
    line.call({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": params_extra()})
    line.call({"jsonrpc": "2.0", "id": 3, "method": "tools/call",
               "params": {"name": "add_notes", **params_extra(),
                          "arguments": {"track": "Piano", "notes": [
                              {"key": 60, "start": 0, "length": 1920, "velocity": 100},
                              {"key": 64, "start": 1920, "length": 1920, "velocity": 100}]}}})
    line.call({"jsonrpc": "2.0", "id": 4, "method": "tools/call",
               "params": {"name": "add_notes", **params_extra(),
                          "arguments": {"track": "Bass", "notes": []}}})
    line.call({"jsonrpc": "2.0", "id": 5, "method": "tools/call",
               "params": {"name": "song_overview", **params_extra(), "arguments": {}}})


def legacy(line):
    r = line.call({"jsonrpc": "2.0", "id": 1, "method": "initialize",
                   "params": {"protocolVersion": OLD, "capabilities": {}, "clientInfo": CLIENT}})
    if r is None or "error" in r:
        return
    line.send({"jsonrpc": "2.0", "method": "notifications/initialized"})
    tool_calls(line, lambda: {})


def new(line):
    r = line.call({"jsonrpc": "2.0", "id": 1, "method": "server/discover", "params": {"_meta": meta()}})
    if r is None or "error" in r:
        return
    tool_calls(line, lambda: {"_meta": meta()})


def main():
    mode, rest = sys.argv[1], sys.argv[2:]
    line = Line(rest)
    {"legacy": legacy, "new": new}[mode](line)
    line.close()


if __name__ == "__main__":
    main()
