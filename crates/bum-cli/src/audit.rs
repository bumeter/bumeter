//! `bum audit`: so tổng token của pipeline với một cách đếm độc lập, đọc thẳng từ log (spec mục 12).
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;
use bum_adapters::claude_code::ClaudeCode;
use bum_core::engine::Engine;
use bum_core::store::Store;

/// [input, cache_creation, cache_read, output], lấy max theo requestId.
fn independent_totals(root: &Path) -> [u64; 4] {
    let mut by_request: HashMap<String, [u64; 4]> = HashMap::new();
    let mut files = Vec::new();
    walk(root, &mut files);
    for f in files {
        let Ok(text) = std::fs::read_to_string(&f) else { continue };
        for line in text.lines() {
            let Ok(v) = serde_json::from_str::<Value>(line) else { continue };
            if v["type"] != "assistant" || v["error"] == "rate_limit" {
                continue;
            }
            let model = v["message"]["model"].as_str().unwrap_or_default();
            if model.is_empty() || model == "<synthetic>" {
                continue;
            }
            let Some(id) = v["requestId"].as_str().or(v["message"]["id"].as_str()) else { continue };
            let u = &v["message"]["usage"];
            let n = |k: &str| u[k].as_u64().unwrap_or(0);
            let row = [n("input_tokens"), n("cache_creation_input_tokens"), n("cache_read_input_tokens"), n("output_tokens")];
            let e = by_request.entry(id.to_string()).or_default();
            for i in 0..4 {
                e[i] = e[i].max(row[i]);
            }
        }
    }
    by_request.values().fold([0; 4], |mut acc, r| {
        for i in 0..4 {
            acc[i] += r[i];
        }
        acc
    })
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out);
        } else if p.extension().is_some_and(|x| x == "jsonl") {
            out.push(p);
        }
    }
}

pub fn run(root: &Path) -> anyhow::Result<bool> {
    let started = std::time::Instant::now();
    let mut engine = Engine::new(Store::open_in_memory()?, vec![Box::new(ClaudeCode::new(root))]);
    let report = engine.full_scan()?;
    let scan_time = started.elapsed();
    let u = engine.store().usage_totals()?;
    let ours = [u.input, u.cache_write(), u.cache_read, u.output];
    let theirs = independent_totals(root);

    println!("Thư mục: {}", root.display());
    println!("Quét: {} dòng, {} lỗi parse, {:?}", report.lines, report.parse_errors, scan_time);
    let labels = ["input", "cache_write", "cache_read", "output"];
    for i in 0..4 {
        let mark = if ours[i] == theirs[i] { "OK" } else { "LỆCH" };
        println!("  {:<12} app={:>14} độc lập={:>14}  {mark}", labels[i], ours[i], theirs[i]);
    }
    let ok = ours == theirs;
    println!("Tổng: app={} độc lập={}  {}", u.total(), theirs.iter().sum::<u64>(), if ok { "OK" } else { "LỆCH" });
    Ok(ok)
}
