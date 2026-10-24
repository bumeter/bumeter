fn main() {
    // `tauri::generate_context!` needs frontendDist (@bumeter/ui's dist) at compile time. If `npm install` hasn't been
    // run in app/, create a temporary page so `cargo build/test` still works.
    let dist = std::path::Path::new("../node_modules/@bumeter/ui/dist");
    if !dist.join("index.html").exists() {
        std::fs::create_dir_all(dist).expect("create @bumeter/ui/dist");
        std::fs::write(dist.join("index.html"), "<!doctype html><meta charset=utf-8><p>Run <code>npm install</code> in app/.</p>")
            .expect("write temporary page");
    }
    println!("cargo:rerun-if-changed=../node_modules/@bumeter/ui/dist");
    tauri_build::build()
}
