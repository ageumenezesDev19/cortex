fn main() {
    // `include_dir!` bakes the web bundle into the binary, and cargo has no way
    // of knowing a rebuilt bundle changed anything. Without this the phone keeps
    // being served the version compiled last week.
    println!("cargo:rerun-if-changed=../dist");
    // `dist` is not in git. Without this, `cargo test` on a fresh clone fails to
    // compile before anyone has had a chance to build the frontend.
    let _ = std::fs::create_dir_all("../dist");
    tauri_build::build()
}
