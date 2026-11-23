//! Embeds the icon in the Windows executable (.exe). Has no effect on other systems.
//! `CARGO_CFG_TARGET_OS` refers to the *target* system: this also works when cross-compiling from Linux
//! (which requires `windres`, provided by the mingw-w64 package). If embedding fails, the build simply
//! continues without an icon.
fn main() {
    println!("cargo:rerun-if-changed=packaging/printsearch.ico");
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("packaging/printsearch.ico");
        res.set("ProductName", "PrintSearch");
        res.set("FileDescription", "PrintSearch - 3D model search");
        if let Err(e) = res.compile() {
            println!("cargo:warning=icon not embedded in the .exe: {e}");
        }
    }
}
