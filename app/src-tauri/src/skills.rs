//! Commands for the Skill screen (spec section 8): see `bum_service::ops`, shared with `bum rpc`.
use serde_json::Value;
use tauri::State;

#[tauri::command(async)]
pub fn skills(ops: State<'_, bum_service::ops::Ops>, action: String, params: Option<Value>) -> Result<Value, String> {
    ops.skills(&action, &params.unwrap_or(Value::Null))
}
