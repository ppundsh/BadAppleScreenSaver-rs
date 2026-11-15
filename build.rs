fn main() {
    println!("cargo:rerun-if-changed=app.manifest");
    println!("cargo:rerun-if-changed=assets/black_apple.ico");
    println!("cargo:rerun-if-changed=assets/black_apple_64.rgba");
    println!("cargo:rerun-if-changed=assets/bad_apple_4_3.mp4");
    println!("cargo:rerun-if-changed=assets/bad_apple_16_9.mp4");
    println!("cargo:rerun-if-changed=assets/bad_apple.mp3");

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/black_apple.ico");
        res.set("FileDescription", "Bad Apple!! rs");
        res.set("ProductName", "Bad Apple!! rs");
        res.append_rc_content("STRINGTABLE\nBEGIN\n    1 \"Bad Apple!! rs\"\nEND\n");
        let _ = res.compile();
    }
}
