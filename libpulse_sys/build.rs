extern crate bindgen;

use std::env;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    bindgen::builder()
        .header("include/libpulse.h")
        .derive_debug(true)
        .rustified_enum("^pa_.+$")
        .generate().unwrap()
        .write_to_file(Path::new(&out_dir).join("libpulse.rs")).unwrap();
    println!("cargo:rustc-link-lib=pulse");
    println!("cargo:rustc-link-lib=pulse-simple");
}
