//! Minimal ACP v1 client for a fresh, trusted Prime Agent session.
//! Client file/terminal capabilities are not a sandbox: Prime uses its own native tools.
use serde_json::{json, Value};
use super::render::{Rendered, ResultInfo, Step};

pub const INITIALIZE_ID: u64 = 1;
pub const NEW_ID: u64 = 2;
pub const PROMPT_ID: u64 = 3;
pub const CLOSE_ID: u64 = 4;

pub struct Client {
    cwd: String,
    prompt: String,
    pending: u64,
    answer: String,
    pub session_id: Option<String>,
}

#[derive(Default)]
pub struct Action {
    pub send: Option<Value>,
    pub rendered: Rendered,
    pub result: Option<ResultInfo>,
    pub closed: bool,
}

fn request(id: u64, method: &str, params: Value) -> Value {
    json!({"jsonrpc":"2.0","id":id,"method":method,"params":params})
}

impl Client {
    pub fn new(cwd: String, prompt: String) -> Self {
        Self { cwd, prompt, pending: INITIALIZE_ID, session_id: None, answer: String::new() }
    }

    pub fn initialize(&self) -> Value {
        request(INITIALIZE_ID, "initialize", json!({
            "protocolVersion":1,
            "clientCapabilities":{"fs":{"readTextFile":false,"writeTextFile":false},"terminal":false},
            "clientInfo":{"name":"ledger","version":env!("CARGO_PKG_VERSION")}
        }))
    }

    pub fn cancel(&self) -> Option<Value> {
        self.session_id.as_ref().map(|id| json!({"jsonrpc":"2.0","method":"session/cancel","params":{"sessionId":id}}))
    }

    pub fn close(&mut self) -> Option<Value> {
        self.pending = CLOSE_ID;
        self.session_id.as_ref().map(|id| request(CLOSE_ID, "session/close", json!({"sessionId":id})))
    }

    pub fn receive(&mut self, line: &str) -> Result<Action, String> {
        let v: Value = serde_json::from_str(line).map_err(|_| "Prime Agent sent invalid ACP JSON".to_string())?;
        if v["jsonrpc"] != "2.0" { return Err("Prime Agent sent an invalid JSON-RPC envelope".into()); }
        if let Some(method) = v["method"].as_str() {
            if let Some(id) = v.get("id") {
                return Ok(Action { send: Some(json!({"jsonrpc":"2.0","id":id,"error":{"code":-32601,"message":"Ledger does not provide client tools or permission prompts"}})), ..Action::default() });
            }
            if method == "session/update" && self.session_id.as_deref() == v.pointer("/params/sessionId").and_then(Value::as_str) {
                let update = &v["params"]["update"];
                let rendered = render_update(update);
                if update["sessionUpdate"] == "agent_message_chunk" {
                    const MAX_ANSWER_BYTES: usize = 512 * 1024;
                    let room = MAX_ANSWER_BYTES.saturating_sub(self.answer.len());
                    let mut end = rendered.text.len().min(room);
                    while !rendered.text.is_char_boundary(end) { end -= 1; }
                    self.answer.push_str(&rendered.text[..end]);
                }
                return Ok(Action { rendered, ..Action::default() });
            }
            return Ok(Action::default());
        }
        if v.get("id").and_then(Value::as_u64) != Some(self.pending) {
            // A prompt may settle while the close request is in flight.
            return Ok(Action::default());
        }
        if let Some(error) = v.get("error") {
            return Err(format!("Prime Agent ACP: {}", error["message"].as_str().unwrap_or("request failed")));
        }
        let result = v.get("result").ok_or("Prime Agent ACP response has no result")?;
        match self.pending {
            INITIALIZE_ID => {
                if result["protocolVersion"].as_u64() != Some(1) { return Err("Prime Agent does not support ACP version 1".into()); }
                self.pending = NEW_ID;
                Ok(Action { send: Some(request(NEW_ID,"session/new",json!({"cwd":self.cwd,"mcpServers":[]}))), ..Action::default() })
            }
            NEW_ID => {
                let id = result["sessionId"].as_str().filter(|s| !s.is_empty()).ok_or("Prime Agent did not return a session ID")?.to_string();
                self.session_id = Some(id.clone());
                if let Some(actual) = result.pointer("/_meta/ai.primeintellect.prime-agent/cwd/actual").and_then(Value::as_str) {
                    if !same_directory(&self.cwd, actual) { return Err(format!("Prime Agent started in a different folder: {actual}")); }
                }
                self.pending = PROMPT_ID;
                Ok(Action { send: Some(request(PROMPT_ID,"session/prompt",json!({"sessionId":id,"prompt":[{"type":"text","text":self.prompt}]}))), ..Action::default() })
            }
            PROMPT_ID => {
                let stop = result["stopReason"].as_str().ok_or("Prime Agent prompt response has no stop reason")?;
                let info = match stop {
                    "end_turn" => ResultInfo { text:self.answer.clone(), ..ResultInfo::default() },
                    "cancelled" => ResultInfo { is_error:true, text:"Prime Agent cancelled the prompt".into(), ..ResultInfo::default() },
                    "max_tokens" | "max_turn_requests" | "refusal" => ResultInfo { is_error:true, text:format!("Prime Agent stopped before completing the prompt: {stop}"), ..ResultInfo::default() },
                    _ => return Err(format!("Prime Agent returned an unknown stop reason: {stop}")),
                };
                Ok(Action { result: Some(info), ..Action::default() })
            }
            CLOSE_ID => Ok(Action { closed:true, ..Action::default() }),
            _ => unreachable!(),
        }
    }
}

fn same_directory(requested: &str, actual: &str) -> bool {
    if requested == actual { return true; }
    match (std::fs::canonicalize(requested), std::fs::canonicalize(actual)) {
        (Ok(requested), Ok(actual)) => requested == actual,
        _ => false,
    }
}

/// Only assistant text and tool summaries enter the output/record, never raw tool results or reasoning.
pub fn render_update(update: &Value) -> Rendered {
    let mut out = Rendered::default();
    match update["sessionUpdate"].as_str().unwrap_or_default() {
        "agent_message_chunk" if update["content"]["type"] == "text" => {
            out.text = update["content"]["text"].as_str().unwrap_or_default().to_string();
        }
        "tool_call" => {
            let name: String = update["title"].as_str().unwrap_or("tool").chars().take(120).collect();
            out.text = format!("\n→ {name}\n");
            out.steps.push(Step { name, detail:String::new(), failed:update["status"] == "failed" });
        }
        "tool_call_update" if update["status"] == "failed" => {
            out.text = "  ✗ Tool call failed\n".into();
            out.steps.push(Step { name:String::new(), detail:"Tool call failed".into(), failed:true });
        }
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn reply(id: u64, result: Value) -> String {
        json!({"jsonrpc":"2.0","id":id,"result":result}).to_string()
    }

    fn ready() -> Client {
        let mut c = Client::new("/project".into(), "do work".into());
        let new = c.receive(&reply(1,json!({"protocolVersion":1}))).unwrap().send.unwrap();
        assert_eq!(new["method"], "session/new");
        assert_eq!(new["params"]["cwd"], "/project");
        let prompt = c.receive(&reply(2,json!({"sessionId":"session"}))).unwrap().send.unwrap();
        assert_eq!(prompt["method"], "session/prompt");
        assert_eq!(prompt["params"]["prompt"][0]["text"], "do work");
        c
    }

    #[test]
    fn prompt_response_not_updates_completes_and_close_is_scoped() {
        let mut c = ready();
        let update = json!({"jsonrpc":"2.0","method":"session/update","params":{"sessionId":"session","update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"answer"}}}});
        let action = c.receive(&update.to_string()).unwrap();
        assert_eq!(action.rendered.text,"answer");
        assert!(action.result.is_none());
        assert!(!c.receive(&reply(3,json!({"stopReason":"end_turn"}))).unwrap().result.unwrap().is_error);
        assert_eq!(c.close().unwrap()["method"],"session/close");
        assert!(c.receive(&reply(4,json!({}))).unwrap().closed);
    }

    #[test]
    fn cancellation_notification_and_failed_stop_reasons() {
        let mut c = ready();
        let cancel = c.cancel().unwrap();
        assert_eq!(cancel["method"],"session/cancel");
        assert!(cancel.get("id").is_none());
        assert_eq!(cancel["params"]["sessionId"],"session");
        for stop in ["cancelled","refusal","max_tokens","max_turn_requests"] {
            assert!(c.receive(&reply(3,json!({"stopReason":stop}))).unwrap().result.unwrap().is_error);
        }
    }

    #[test]
    fn invalid_and_error_responses_never_succeed() {
        let mut c = ready();
        assert!(c.receive("not json").is_err());
        assert!(c.receive(&reply(3,json!({}))).is_err());
        assert!(c.receive(&reply(3,json!({"stopReason":"mystery"}))).is_err());
        let err = json!({"jsonrpc":"2.0","id":3,"error":{"code":-32603,"message":"inference failed"}});
        assert!(c.receive(&err.to_string()).err().unwrap().contains("inference failed"));
    }

    #[test]
    fn no_client_tools_and_no_tool_output_or_reasoning_retained() {
        let mut c = ready();
        let reverse = json!({"jsonrpc":"2.0","id":"reverse","method":"fs/read_text_file","params":{"path":"secret"}});
        let denied = c.receive(&reverse.to_string()).unwrap().send.unwrap();
        assert_eq!(denied["error"]["code"],-32601);
        for update in [json!({"sessionUpdate":"agent_thought_chunk","content":{"type":"text","text":"secret"}}),json!({"sessionUpdate":"tool_call_update","status":"completed","content":[{"text":"secret"}]})] {
            assert!(render_update(&update).text.is_empty());
        }
    }
}
