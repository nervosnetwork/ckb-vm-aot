// Due to this bug: https://github.com/rust-lang/cargo/issues/4866, we cannot
// specify different features based on different targets now in cargo file. We
// have to keep features always on, and do conditional compilation within the
// source code

use cc::Build;

fn main() {
    let mut build = Build::new();
    build.file("src/aot.x64.compiled.c");
    build.include("dynasm");
    build.compile("aot");
}
