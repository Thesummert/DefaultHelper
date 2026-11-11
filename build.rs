fn main() {
    slint_build::compile("ui/app.slint").expect("failed to compile Slint UI");
    println!("cargo:rerun-if-changed=src/macos.m");
    println!("cargo:rerun-if-changed=src/linux.c");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux") {
        // Emit the native archive before its shared-library dependencies.
        let gio = pkg_config::Config::new()
            .atleast_version("2.66")
            .cargo_metadata(false)
            .probe("gio-unix-2.0")
            .expect("Linux requires GLib/GIO development files and pkg-config (libglib2.0-dev on Debian/Ubuntu, glib2-devel on Fedora, glib2 on Arch)");
        cc::Build::new()
            .file("src/linux.c")
            .includes(&gio.include_paths)
            .warnings(true)
            .compile("default_helper_linux");
        pkg_config::Config::new().probe("gio-unix-2.0").unwrap();
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        cc::Build::new()
            .file("src/macos.m")
            .flag("-fobjc-arc")
            .flag("-fblocks")
            .compile("default_helper_macos");
        println!("cargo:rustc-link-lib=framework=AppKit");
        println!("cargo:rustc-link-lib=framework=CoreServices");
        println!("cargo:rustc-link-arg=-Wl,-weak_framework,UniformTypeIdentifiers");
    }
}
