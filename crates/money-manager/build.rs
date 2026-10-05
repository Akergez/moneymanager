//! Puts the icon into the Windows executable.
//!
//! Windows takes a program's icon from the resources linked into its .exe,
//! and GPUI asks for resource 1 when it makes a window. With nothing linked
//! in, the taskbar shows a blank. Nothing happens for any other target.
//!
//! The icon is a file in the repository rather than something drawn here from
//! the SVG: regenerate `windows/money-manager.ico` when the SVG changes, with
//!
//! ```sh
//! magick -background none -density 384 \
//!   data/icons/hicolor/scalable/apps/app.akergez.MoneyManager.svg \
//!   -define icon:auto-resize=256,64,48,32,24,16 \
//!   crates/money-manager/windows/money-manager.ico
//! ```

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let resources = std::path::Path::new("windows");
    let rc = resources.join("money-manager.rc");
    println!("cargo:rerun-if-changed={}", rc.display());
    println!(
        "cargo:rerun-if-changed={}",
        resources.join("money-manager.ico").display()
    );
    // No resource compiler is no icon, not no build; one that is there and
    // fails is a mistake in the file, and that should be heard.
    embed_resource::compile(rc, embed_resource::ParamsIncludeDirs([resources]))
        .manifest_optional()
        .unwrap();
}
