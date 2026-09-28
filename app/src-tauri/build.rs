fn main() {
    // `tauri::generate_context!` cần `ui/dist` lúc compile. Chưa chạy `npm run build` thì tạo trang tạm
    // để `cargo build/test` của cả workspace vẫn chạy được.
    let dist = std::path::Path::new("../ui/dist");
    if !dist.join("index.html").exists() {
        std::fs::create_dir_all(dist).expect("tạo ui/dist");
        std::fs::write(dist.join("index.html"), "<!doctype html><meta charset=utf-8><p>Chạy <code>npm run build</code> trong app/ui.</p>")
            .expect("ghi trang tạm");
    }
    println!("cargo:rerun-if-changed=../ui/dist");
    tauri_build::build()
}
