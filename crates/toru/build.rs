fn main() {
    println!("cargo:rerun-if-changed=../../assets/toru.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    // `set_icon` stores the icon as resource 1, the one GPUI loads for the window and the taskbar.
    // Task Manager and "Open with" show FileDescription as the app's name.
    winresource::WindowsResource::new()
        .set_icon("../../assets/toru.ico")
        .set("FileDescription", "Toru")
        .set("ProductName", "Toru")
        .compile()
        .expect("rc.exe compiles toru.exe's icon and version resource");
}
