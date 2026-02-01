use std::env;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    
    // If we aren't compiling FOR Windows, skip everything.
    if target_os != "windows" {
        return;
    }

    let mut res = winres::WindowsResource::new();

    // ONLY set these paths if we are compiling FROM a non-Windows machine (e.g., Linux)
    // The build script itself runs on the host, so cfg!(windows) checks the host OS.
    if !cfg!(windows) {
        res.set_toolkit_path("/usr/bin");
        res.set_ar_path("x86_64-w64-mingw32-ar");
        res.set_windres_path("x86_64-w64-mingw32-windres");
    }

    res.set_icon("icon.ico");
    res.compile().expect("failed to compile Windows resources");
}