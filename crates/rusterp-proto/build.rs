use std::env;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let proto_root = manifest_dir.join("../../proto");
    let proto_root = proto_root.canonicalize()?;

    let protos = [
        proto_root.join("rusterp/party/v1/party.proto"),
        proto_root.join("rusterp/platform/v1/health.proto"),
        proto_root.join("rusterp/platform/v1/modules_auth.proto"),
        proto_root.join("rusterp/catalog/v1/catalog.proto"),
        proto_root.join("rusterp/sales/v1/sales.proto"),
        proto_root.join("rusterp/payment/v1/payment.proto"),
        proto_root.join("rusterp/inventory/v1/inventory.proto"),
    ];

    for p in &protos {
        println!("cargo:rerun-if-changed={}", p.display());
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let descriptor_path = out_dir.join("rusterp_descriptor.bin");

    tonic_prost_build::configure()
        .build_server(true)
        .build_client(true)
        .file_descriptor_set_path(&descriptor_path)
        .compile_protos(&protos, &[proto_root])?;

    Ok(())
}
