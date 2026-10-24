//! A single command for the UI, with the same set of commands as bum-service (spec section 14.4).
use serde_json::Value;
use tauri::State;
use bum_core::handlers::{dispatch, HandlerCtx};
use bum_core::store::Store;

use crate::state::AppState;

/// `async`: runs off the main thread so DB queries don't freeze the window.
#[tauri::command(async)]
pub fn call(state: State<'_, AppState>, method: String, params: Option<Value>) -> Result<Value, String> {
    #[cfg(debug_assertions)]
    eprintln!("bumeter: ui call {method}");
    let store = Store::open_readonly(&state.paths.db()).map_err(|e| e.to_string())?;
    let params = params.unwrap_or(Value::Null);
    // Session analysis, transcripts and sources read the tools' logs (adapters), which the core cannot.
    if let Some(r) = bum_service::extra::call(&store, &state.cfg, &method, &params) {
        return r;
    }
    let ctx = HandlerCtx { store: &store, pricing: &state.pricing, cfg: &state.cfg, paths: &state.paths };
    dispatch(&ctx, &method, &params, chrono::Utc::now())
}
