//! Imports the types used in REST communication with the server

/// Types for messages to server
pub mod types {
    include!("../../openapi/types.rs");
}

#[macro_use]
extern crate packed_struct;
