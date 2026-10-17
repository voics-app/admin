//! Ensures the SPA embed folder exists so the BFF builds before (or without) the web build.
//! A BFF built this way answers SPA routes with 503 until rebuilt after `web` is built.

fn main() {
    let dist = std::path::Path::new("../web/dist");
    if !dist.exists() {
        std::fs::create_dir_all(dist).expect("create ../web/dist");
    }
    println!("cargo:rerun-if-changed=../web/dist");
    println!("cargo:rerun-if-changed=migrations");
}
