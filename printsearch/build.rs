//! 1. Embeds the icon in the Windows executable (.exe). Has no effect on other systems.
//!    `CARGO_CFG_TARGET_OS` refers to the *target* system: this also works when cross-compiling from Linux
//!    (which requires `windres`, provided by the mingw-w64 package). If embedding fails, the build simply
//!    continues without an icon.
//! 2. Enables `has_tray` when the tray icon is available ("tray" feature + Windows or Linux).
fn main() {
    println!("cargo:rerun-if-changed=packaging/printsearch.ico");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rustc-check-cfg=cfg(has_tray)");

    let os = std::env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if std::env::var_os("CARGO_FEATURE_TRAY").is_some() && (os == "windows" || os == "linux") {
        println!("cargo:rustc-cfg=has_tray");
    }

    if os == "windows" {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("packaging/printsearch.ico");
        res.set("ProductName", "PrintSearch");
        res.set("FileDescription", "PrintSearch - 3D model search");
        if let Err(e) = res.compile() {
            println!("cargo:warning=icon not embedded in the .exe: {e}");
        }
    }
}
