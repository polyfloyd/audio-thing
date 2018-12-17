extern crate bindgen;

use std::env;
use std::path::Path;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    bindgen::builder()
        .header("/usr/include/FLAC/all.h")
        .derive_debug(true)
        .rustified_enum("^FLAC__.+$")
        .generate()
        .unwrap()
        .write_to_file(Path::new(&out_dir).join("libflac.rs"))
        .unwrap();
    println!("cargo:rustc-link-lib=FLAC");
}
