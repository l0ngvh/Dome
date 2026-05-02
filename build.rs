// Dome Windows resource build script.
//
// On Windows targets, compiles `resources/windows/dome.rc` into a linkable
// resource object via the `embed-resource` crate. This embeds the PMv2
// application manifest (so DPI awareness is set before any DLL code runs)
// and the application icon into dome.exe.
//
// On non-Windows targets, this script is a no-op.
//
// Cross-compile prerequisite (macOS -> x86_64-pc-windows-gnu):
// `embed-resource` invokes `windres` for GNU targets. Install mingw-w64
// (`brew install mingw-w64`) so `x86_64-w64-mingw32-windres` is on PATH.
// Without it, the build fails fast per the `.manifest_required()` call below.

use std::env;

fn main() {
    println!("cargo:rerun-if-changed=resources/windows/dome.rc");
    println!("cargo:rerun-if-changed=resources/windows/dome.manifest");
    println!("cargo:rerun-if-changed=resources/windows/Dome.ico");

    let target_os =
        env::var("CARGO_CFG_TARGET_OS").expect("CARGO_CFG_TARGET_OS must be set by cargo");
    if target_os != "windows" {
        return;
    }

    // `manifest_required()` turns a missing windres/rc.exe into a build error
    // instead of silently producing an EXE with no manifest. This matches the
    // project's fail-fast principle (AGENTS.md).
    embed_resource::compile("resources/windows/dome.rc", embed_resource::NONE)
        .manifest_required()
        .unwrap();
}
