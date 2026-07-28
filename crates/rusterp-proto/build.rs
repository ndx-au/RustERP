use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let proto_root = manifest_dir.join("../../proto");
    let proto_root = proto_root.canonicalize()?;

    let party = proto_root.join("rusterp/party/v1/party.proto");
    let health = proto_root.join("rusterp/platform/v1/health.proto");

    println!("cargo:rerun-if-changed={}", party.display());
    println!("cargo:rerun-if-changed={}", health.display());

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let descriptor_path = out_dir.join("rusterp_descriptor.bin");

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(&descriptor_path)
        .compile_protos(&[party, health], &[proto_root])?;

    Ok(())
}
