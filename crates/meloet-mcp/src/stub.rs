//! The answers given while meloet is away (closed, or 「AI とつなぐ」 off): a small MCP
//! server with one tool, so that the model can tell the person what to do and the AI app
//! needs no restart once the link is on. Both handshakes are answered as rmcp 3.11 answers
//! them: `initialize` (2025-11-25 and before) and `server/discover` with `_meta` on every
//! request (2026-07-28).

use serde_json::{Value, json};

/// The words of the spec (A-27) for where the switch is.
pub const OFF_TEXT: &str =
    "meloet で「音の設定 ▸ くわしい設定 ▸ AI とつなぐ」をオンにしてください。";

pub const STATUS_TOOL: &str = "meloet_status";

const VERSIONS: [&str; 5] = [
    "2024-11-05",
    "2025-03-26",
    "2025-06-18",
    "2025-11-25",
    "2026-07-28",
];

const PROTOCOL_KEY: &str = "io.modelcontextprotocol/protocolVersion";
const CAPABILITIES_KEY: &str = "io.modelcontextprotocol/clientCapabilities";

pub fn server_info() -> Value {
    json!({ "name": "meloet", "version": env!("CARGO_PKG_VERSION") })
}

fn capabilities() -> Value {
    json!({ "tools": { "listChanged": true } })
}

/// The request carries the protocol version in `_meta`: the 2026-07-28 way.
pub fn is_new(msg: &Value) -> bool {
    msg["params"]["_meta"][PROTOCOL_KEY].is_string()
}

fn has_meta(msg: &Value) -> bool {
    is_new(msg) && msg["params"]["_meta"][CAPABILITIES_KEY].is_object()
}

pub fn result(id: &Value, result: Value) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "result": result }).to_string()
}

pub fn error(id: &Value, code: i64, message: &str) -> String {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } }).to_string()
}

/// The answer to a call of any tool while meloet is away.
pub fn off_call(new: bool) -> Value {
    complete(
        new,
        json!({ "content": [{ "type": "text", "text": OFF_TEXT }], "isError": true }),
    )
}

/// The answer to the status tool once meloet is there again.
pub fn back_call(new: bool) -> Value {
    complete(
        new,
        json!({
            "content": [{
                "type": "text",
                "text": "meloet is connected now; its tools were listed again \
                         (notifications/tools/list_changed). If they do not show, ask the \
                         person to reload the MCP servers of the AI app.",
            }],
            "isError": false,
        }),
    )
}

fn complete(new: bool, mut value: Value) -> Value {
    if new {
        value["resultType"] = json!("complete");
    }
    value
}

/// The answer to a request while meloet is away; `None` for a notification.
pub fn answer(msg: &Value) -> Option<String> {
    let id = msg.get("id")?;
    let method = msg["method"].as_str()?;
    let new = is_new(msg);
    Some(match method {
        "initialize" => result(
            id,
            json!({
                "protocolVersion": msg["params"]["protocolVersion"],
                "capabilities": capabilities(),
                "serverInfo": server_info(),
                "instructions": OFF_TEXT,
            }),
        ),
        "server/discover" if !has_meta(msg) => error(
            id,
            -32602,
            "missing _meta: io.modelcontextprotocol/protocolVersion, \
             io.modelcontextprotocol/clientCapabilities",
        ),
        "server/discover" => result(
            id,
            json!({
                "resultType": "complete",
                "supportedVersions": VERSIONS,
                "capabilities": capabilities(),
                "instructions": OFF_TEXT,
                "ttlMs": 0,
                "cacheScope": "private",
                "_meta": { "io.modelcontextprotocol/serverInfo": server_info() },
            }),
        ),
        "ping" => result(id, json!({})),
        "tools/list" => {
            let mut list = json!({
                "tools": [{
                    "name": STATUS_TOOL,
                    "title": "meloet status",
                    "description": "Says whether meloet can be reached. meloet's own tools \
                                    (reading and writing the song) show once it can.",
                    "inputSchema": { "type": "object", "properties": {} },
                    "annotations": { "readOnlyHint": true, "openWorldHint": false },
                }],
            });
            if new {
                list["resultType"] = json!("complete");
                list["ttlMs"] = json!(0);
                list["cacheScope"] = json!("public");
            }
            result(id, list)
        }
        "tools/call" => result(id, off_call(new)),
        _ => error(id, -32601, OFF_TEXT),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(line: &str) -> Value {
        serde_json::from_str(line).unwrap()
    }

    #[test]
    fn the_old_handshake_is_answered() {
        let init = json!({"jsonrpc":"2.0","id":1,"method":"initialize",
            "params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"t","version":"0"}}});
        let a = parse(&answer(&init).unwrap());
        assert_eq!(a["result"]["protocolVersion"], "2025-11-25");
        assert_eq!(a["result"]["capabilities"]["tools"]["listChanged"], true);
        assert_eq!(a["result"]["instructions"], OFF_TEXT);
        assert!(answer(&json!({"jsonrpc":"2.0","method":"notifications/initialized"})).is_none());
        let list = parse(&answer(&json!({"jsonrpc":"2.0","id":2,"method":"tools/list"})).unwrap());
        assert_eq!(list["result"]["tools"][0]["name"], STATUS_TOOL);
        assert!(list["result"].get("resultType").is_none());
        let call = json!({"jsonrpc":"2.0","id":3,"method":"tools/call",
            "params":{"name":"song_overview","arguments":{}}});
        let call = parse(&answer(&call).unwrap());
        assert_eq!(call["result"]["isError"], true);
        assert_eq!(call["result"]["content"][0]["text"], OFF_TEXT);
    }

    #[test]
    fn the_new_handshake_is_answered() {
        let meta = json!({PROTOCOL_KEY: "2026-07-28", CAPABILITIES_KEY: {}});
        let discover =
            json!({"jsonrpc":"2.0","id":"d","method":"server/discover","params":{"_meta":meta}});
        let a = parse(&answer(&discover).unwrap());
        assert_eq!(a["result"]["resultType"], "complete");
        assert!(
            a["result"]["supportedVersions"]
                .as_array()
                .unwrap()
                .contains(&json!("2026-07-28"))
        );
        assert_eq!(
            a["result"]["_meta"]["io.modelcontextprotocol/serverInfo"]["name"],
            "meloet"
        );
        let list = json!({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{"_meta":meta}});
        let list = parse(&answer(&list).unwrap());
        assert_eq!(list["result"]["ttlMs"], 0);
        assert_eq!(list["result"]["resultType"], "complete");
        // A discover without its `_meta` is refused, as rmcp does; `initialize` still works.
        let bare = json!({"jsonrpc":"2.0","id":4,"method":"server/discover"});
        assert_eq!(parse(&answer(&bare).unwrap())["error"]["code"], -32602);
    }
}
