use cc::Build;
use std::env;

fn main() {
    let target_family = env::var("CARGO_CFG_TARGET_FAMILY").unwrap_or_default();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap_or_default();
    let mut build = Build::new();
    if target_arch == "x86_64" && target_family == "windows" {
        build.file("src/aot.x64.win.compiled.c");
        build.include("dynasm");
        build.compile("aot");
        println!("cargo:rustc-cfg=has_aot");
    }
    if target_arch == "x86_64" && target_family == "unix" {
        build.file("src/aot.x64.compiled.c");
        build.include("dynasm");
        build.compile("aot");
        println!("cargo:rustc-cfg=has_aot");
    }
}
