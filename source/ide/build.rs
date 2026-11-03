//! Embeds the application icon into `nortex-code.exe` (Windows hosts only).

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=nortex.manifest");
    println!("cargo:rerun-if-changed=../assets/icons/app/nortex-code.ico");
    embed_windows_icon();
}

#[cfg(windows)]
fn embed_windows_icon() {
    let mut resource = winres::WindowsResource::new();
    resource.set_icon("../assets/icons/app/nortex-code.ico");
    resource.set("ProductName", "NORTEX Code");
    resource.set("FileDescription", "NORTEX Code");
    resource.set_manifest(include_str!("nortex.manifest"));
    // A missing resource compiler must not break the build; only the Explorer icon is lost.
    if let Err(error) = resource.compile() {
        println!("cargo:warning=Windows-Icon wurde nicht eingebettet: {error}");
    }
}

#[cfg(not(windows))]
fn embed_windows_icon() {}
