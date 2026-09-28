//! Một lệnh duy nhất cho UI, cùng bộ lệnh với tk-service (spec mục 14.4).
use serde_json::Value;
use tauri::State;
use tk_core::handlers::{dispatch, HandlerCtx};
use tk_core::store::Store;

use crate::state::AppState;

/// `async`: chạy ngoài main thread để truy vấn DB không làm đơ cửa sổ.
#[tauri::command(async)]
pub fn call(state: State<'_, AppState>, method: String, params: Option<Value>) -> Result<Value, String> {
    #[cfg(debug_assertions)]
    eprintln!("tk: ui call {method}");
    let store = Store::open_readonly(&state.paths.db()).map_err(|e| e.to_string())?;
    // Phân tích phiên cần đọc log Claude trực tiếp (adapter), không qua dispatch của lõi.
    let params = params.unwrap_or(Value::Null);
    if method == "session_analysis" || method == "call_detail" {
        use tk_adapters::claude_code::composition::{analyze_file, call_detail};
        let sid = params["session_id"].as_str().ok_or("thiếu 'session_id'")?;
        let file = store.session_file(sid).map_err(|e| e.to_string())?.ok_or("không tìm thấy file log của phiên")?;
        let path = std::path::Path::new(&file);
        return if method == "session_analysis" {
            analyze_file(path).map(|a| serde_json::to_value(a).unwrap_or_default()).map_err(|e| e.to_string())
        } else {
            let rid = params["request_id"].as_str().ok_or("thiếu 'request_id'")?;
            call_detail(path, rid).map(|d| serde_json::to_value(d).unwrap_or_default()).ok_or_else(|| "không có lượt gọi này".into())
        };
    }
    if method == "conversation_transcript" {
        let sid = params["session_id"].as_str().ok_or("thiếu 'session_id'")?;
        let file = store.session_file(sid).map_err(|e| e.to_string())?.ok_or("không tìm thấy file log của phiên")?;
        let tool = store.session_tool(sid).map_err(|e| e.to_string())?.unwrap_or_default();
        let path = std::path::Path::new(&file);
        let turns = match tool.as_str() {
            tk_adapters::kiro::TOOL_ID => tk_adapters::kiro::transcript(path),
            _ => tk_adapters::claude_code::transcript::read(path),
        }
        .map_err(|e| e.to_string())?;
        let turns = tk_core::transcript::only(turns, params["key"].as_str());
        return Ok(serde_json::to_value(turns).unwrap_or_default());
    }
    if method == "sources" {
        let roots = tk_adapters::registry(&tk_service::host::claude_root(&state.cfg), &tk_service::host::kiro_root(&state.cfg));
        let stats = tk_core::handlers::tool_stats(&store).map_err(|e| e.to_string())?;
        return Ok(sources_json(roots, &stats));
    }
    let ctx = HandlerCtx { store: &store, pricing: &state.pricing, cfg: &state.cfg, paths: &state.paths };
    dispatch(&ctx, &method, &params, chrono::Utc::now())
}

/// Mỗi công cụ: mô tả từ adapter + trạng thái (thư mục log có không, số phiên, lượt, lần cuối).
fn sources_json(roots: Vec<(tk_core::source::SourceInfo, std::path::PathBuf)>, stats: &[tk_core::handlers::ToolStat]) -> Value {
    let list: Vec<Value> = roots
        .into_iter()
        .map(|(info, root)| {
            let st = stats.iter().find(|s| s.tool == info.id);
            serde_json::json!({
                "info": info,
                "root": root.to_string_lossy(),
                "found": root.is_dir(),
                "sessions": st.map_or(0, |s| s.sessions),
                "prompts": st.map_or(0, |s| s.prompts),
                "last_ts": st.and_then(|s| s.last_ts.clone()),
            })
        })
        .collect();
    Value::Array(list)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sources_merge_registry_with_stats() {
        let reg = tk_adapters::registry(std::path::Path::new("/nonexistent-c"), std::path::Path::new("/nonexistent-k"));
        let stats = vec![tk_core::handlers::ToolStat { tool: "kiro".into(), sessions: 2, prompts: 5, last_ts: Some("t".into()) }];
        let v = sources_json(reg, &stats);
        assert_eq!(v[0]["info"]["id"], "claude_code");
        assert_eq!(v[0]["sessions"], 0);
        assert_eq!(v[0]["found"], false);
        assert_eq!((v[1]["sessions"].as_u64(), v[1]["prompts"].as_u64(), v[1]["last_ts"].as_str()), (Some(2), Some(5), Some("t")));
    }
}
