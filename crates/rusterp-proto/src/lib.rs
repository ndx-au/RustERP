//! Generated protobuf / gRPC types for RustERP.
//!
//! Sources live under the workspace `proto/` tree. Regenerate by rebuilding this
//! crate (`cargo build -p rusterp-proto`); `build.rs` invokes `tonic-prost-build`
//! against those `.proto` files (requires `protoc` on `PATH`).

/// Encoded `FileDescriptorSet` for gRPC server reflection.
pub const FILE_DESCRIPTOR_SET: &[u8] =
    tonic::include_file_descriptor_set!("rusterp_descriptor");

pub mod party {
    pub mod v1 {
        tonic::include_proto!("rusterp.party.v1");
    }
}

pub mod platform {
    pub mod v1 {
        tonic::include_proto!("rusterp.platform.v1");
    }
}

pub mod catalog {
    pub mod v1 {
        tonic::include_proto!("rusterp.catalog.v1");
    }
}

pub mod sales {
    pub mod v1 {
        tonic::include_proto!("rusterp.sales.v1");
    }
}

pub mod payment {
    pub mod v1 {
        tonic::include_proto!("rusterp.payment.v1");
    }
}

pub mod inventory {
    pub mod v1 {
        tonic::include_proto!("rusterp.inventory.v1");
    }
}
