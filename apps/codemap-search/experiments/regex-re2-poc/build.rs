use std::{env, path::PathBuf};

fn main() {
    let native = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap()).join(".cache/native");
    println!("cargo:rerun-if-changed=native/bridge.cc");
    println!("cargo:rerun-if-changed={}", native.display());
    println!("cargo:rustc-link-search=native={}", native.display());
    println!("cargo:rustc-link-lib=dylib=regex_poc_re2");
    println!("cargo:rustc-link-arg=-Wl,-rpath,{}", native.display());
}
