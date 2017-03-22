extern crate bindgen;

use std::env;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    bindgen::builder()
        .header("header/pulse.h")
        .no_unstable_rust()
        .derive_debug(true)
        .generate().unwrap()
        .write_to_file(Path::new(&out_dir).join("pulse.rs")).unwrap();
    println!("cargo:rustc-link-lib=pulse");
    println!("cargo:rustc-link-lib=pulse-simple");

    bindgen::builder()
        .header("/usr/include/x86_64-linux-gnu/libswresample/swresample.h")
        .no_unstable_rust()
        .derive_debug(true)
        .generate().unwrap()
        .write_to_file(Path::new(&out_dir).join("swresample.rs")).unwrap();
    println!("cargo:rustc-link-lib=swresample");
    println!("cargo:rustc-link-lib=avutil");
}
