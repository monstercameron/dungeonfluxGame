//! Generated common contracts and explicitly experimental S00 fixture contracts.
pub mod transport_fixture {
    tonic::include_proto!("dungeonflux.experimental.transport.v1");
}

pub mod public {
    pub mod v1 {
        tonic::include_proto!("dungeonflux.public.v1");
    }
}

pub mod experimental {
    pub mod contract {
        pub mod v1 {
            tonic::include_proto!("dungeonflux.experimental.contract.v1");
        }
    }
}

pub use experimental::contract::v1 as contract_fixture;
pub use public::v1 as common;

/// Actual protoc descriptor bytes for the generated schemas, including field presence.
pub const FILE_DESCRIPTOR_SET: &[u8] = tonic::include_file_descriptor_set!("contracts");
