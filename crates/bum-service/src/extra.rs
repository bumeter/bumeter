//! Methods that read the tools' own logs (through the adapters), which `bum_core::handlers::dispatch`
//! cannot do: the core does not know the adapters. Shared by the app and `bum rpc`.
use serde_json::Value;
use bum_core::config::Config;
use bum_core::store::Store;

/// `None` when `method` is not one of these (the caller then uses `dispatch`).
pub fn call(store: &Store, cfg: &Config, method: &str, params: &Value) -> Option<Result<Value, String>> {
    match method {
        "session_analysis" | "call_detail" => Some(session(store, method, params)),
        "conversation_transcript" => Some(transcript(store, params)),
        "sources" => Some(sources(store, cfg)),
        _ => None,
    }
}

fn session(store: &Store, method: &str, params: &Value) -> Result<Value, String> {
    use bum_adapters::claude_code::composition::{analyze_file, call_detail};
    let sid = params["session_id"].as_str().ok_or("missing 'session_id'")?;
    let file = store.session_file(sid).map_err(|e| e.to_string())?.ok_or("session log file not found")?;
    if file.starts_with("db:") {
        return Err("context analysis is not available for this tool yet".into());
    }
    let path = std::path::Path::new(&file);
    if method == "session_analysis" {
        analyze_file(path).map(|a| serde_json::to_value(a).unwrap_or_default()).map_err(|e| e.to_string())
    } else {
        let rid = params["request_id"].as_str().ok_or("missing 'request_id'")?;
        call_detail(path, rid).map(|d| serde_json::to_value(d).unwrap_or_default()).ok_or_else(|| "no such call".into())
    }
}

fn transcript(store: &Store, params: &Value) -> Result<Value, String> {
    let sid = params["session_id"].as_str().ok_or("missing 'session_id'")?;
    let file = store.session_file(sid).map_err(|e| e.to_string())?.ok_or("session log file not found")?;
    let tool = store.session_tool(sid).map_err(|e| e.to_string())?.unwrap_or_default();
    if file.starts_with("db:") {
        return Err("the conversation transcript is not available for this tool yet".into());
    }
    let path = std::path::Path::new(&file);
    let turns = match tool.as_str() {
        bum_adapters::kiro::TOOL_ID => bum_adapters::kiro::transcript(path),
        _ => bum_adapters::claude_code::transcript::read(path),
    }
    .map_err(|e| e.to_string())?;
    let turns = bum_core::transcript::only(turns, params["key"].as_str());
    Ok(serde_json::to_value(turns).unwrap_or_default())
}

/// Per tool: description from the adapter + state (whether its data exists — a log directory, or a
/// database file for OpenCode —, number of sessions, prompts, last time).
fn sources(store: &Store, cfg: &Config) -> Result<Value, String> {
    let roots = bum_adapters::registry(&crate::host::roots(cfg));
    let stats = bum_core::handlers::tool_stats(store).map_err(|e| e.to_string())?;
    let list: Vec<Value> = roots
        .into_iter()
        .map(|(info, root)| {
            let st = stats.iter().find(|s| s.tool == info.id);
            serde_json::json!({
                "info": info,
                "root": root.to_string_lossy(),
                "found": root.exists(),
                "sessions": st.map_or(0, |s| s.sessions),
                "prompts": st.map_or(0, |s| s.prompts),
                "last_ts": st.and_then(|s| s.last_ts.clone()),
            })
        })
        .collect();
    Ok(Value::Array(list))
}
