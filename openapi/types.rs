/// Types used for REST communication with the server
// mod status;
// mod error;
// mod warning;

// use status::Status;
// use error::Errors;
// use warning::Warnings;

use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;
use ordered_float::OrderedFloat;

/// Reduced telemetry from the asset
/// Reduced bandwidth for nominal flight
#[allow(dead_code)]
#[derive(Debug, Copy, Clone, IntoParams, ToSchema)]
#[derive(Deserialize, Serialize)]
pub struct BasicTelemetryData {

    /// Asset ID
    pub uuid: Uuid, // 128 bits, can we shrink this?

    /// Current Position
    pub position: PositionData,

    // Active Status
    // pub status: Status,

    // // Active Warnings
    // pub warnings: Warnings,

    // // Active Errors
    // pub errors: Errors,

    /// Data Checksum
    checksum: u32
}

impl BasicTelemetryData {
    #[allow(dead_code)]
    fn checksum_update(&mut self) {
        self.checksum = 0x0;
        todo!();
    }
}

/// Global location of the asset
#[allow(dead_code)]
#[derive(Debug, Clone, IntoParams, ToSchema, Copy)]
#[derive(Deserialize, Serialize)]
pub struct PositionData {
    /// current latitude
    pub latitude: OrderedFloat<f32>,

    /// current longitude
    pub longitude: OrderedFloat<f32>,

    /// current altitude in meters
    pub altitude_meters: OrderedFloat<f32>,
}