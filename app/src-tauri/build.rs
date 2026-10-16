fn main() {
    // `tauri::generate_context!` needs `ui/dist` at compile time. If `npm run build` hasn't been run, create a temporary page
    // so `cargo build/test` for the whole workspace still works.
    let dist = std::path::Path::new("../ui/dist");
    if !dist.join("index.html").exists() {
        std::fs::create_dir_all(dist).expect("create ui/dist");
        std::fs::write(dist.join("index.html"), "<!doctype html><meta charset=utf-8><p>Run <code>npm run build</code> in app/ui.</p>")
            .expect("write temporary page");
    }
    println!("cargo:rerun-if-changed=../ui/dist");
    tauri_build::build()
}
