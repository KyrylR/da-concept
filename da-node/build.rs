fn main() {
    println!("cargo:rerun-if-changed=migrations");
    println!("cargo:rerun-if-changed=proto/sync.proto");

    match tonic_build::configure().compile_protos(&["proto/sync.proto"], &["proto"]) {
        Ok(_) => {}
        Err(e) => println!("cargo:warning=Proto compilation failed: {}", e),
    }
}
