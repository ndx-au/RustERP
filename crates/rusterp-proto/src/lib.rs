//! Generated protobuf / gRPC types for RustERP.
//!
//! Sources live under the workspace `proto/` tree. Regenerate by rebuilding this
//! crate (`cargo build -p rusterp-proto`); `build.rs` invokes `tonic-prost-build`
//! against those `.proto` files (requires `protoc` on `PATH`).
//!
//! Runtime: **tokio** + **tonic** + **prost**.

/// Encoded `FileDescriptorSet` for gRPC server reflection.
pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("rusterp_descriptor");

/// `rusterp.party.v1` — Parties domain messages and `PartyService`.
pub mod party {
    pub mod v1 {
        tonic::include_proto!("rusterp.party.v1");
    }
}

/// `rusterp.platform.v1` — minimal platform surfaces (Health).
pub mod platform {
    pub mod v1 {
        tonic::include_proto!("rusterp.platform.v1");
    }
}
