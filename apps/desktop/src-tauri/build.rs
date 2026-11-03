#[path = "src/notices_build.rs"]
mod notices_build;

fn main() {
    let manifest = std::path::PathBuf::from(std::env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let root = manifest.join("../../..");
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    notices_build::generate(&root, &out).expect("Complete embedded license materials are required");
    tauri_build::build();
}
