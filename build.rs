fn main() {
    slint_build::compile("ui/app.slint").expect("failed to compile Slint UI");
    println!("cargo:rerun-if-changed=src/macos.m");
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
