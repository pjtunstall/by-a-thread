use std::env;

fn main() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os == "windows" {
        let mut res = winres::WindowsResource::new();
        res.set_toolkit_path("/usr/bin");
        res.set_ar_path("x86_64-w64-mingw32-ar");
        res.set_windres_path("x86_64-w64-mingw32-windres");
        res.set_icon("icon.ico");
        res.compile().unwrap();
    }
}
