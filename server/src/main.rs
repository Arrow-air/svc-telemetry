//! gRPC server implementation

#[macro_use]
extern crate packed_struct;

/// Module generated from grpc.proto
pub mod grpc {
    #![allow(unused_qualifications, missing_docs)]
    include!("grpc.rs");
}

use grpc::svc_telemetry_rpc_server::{SvcTelemetryRpc, SvcTelemetryRpcServer};
use grpc::{QueryIsReady, ReadyResponse};
use tonic::{transport::Server, Request, Response, Status};

///Implementation of gRPC endpoints
#[derive(Debug, Default, Copy, Clone)]
pub struct SvcTelemetryImpl {}

#[tonic::async_trait]
impl SvcTelemetryRpc for SvcTelemetryImpl {
    /// Returns ready:true when service is available
    async fn is_ready(
        &self,
        _request: Request<QueryIsReady>,
    ) -> Result<Response<ReadyResponse>, Status> {
        let response = ReadyResponse { ready: true };
        Ok(Response::new(response))
    }
}

///Main entry point: starts gRPC Server on specified address and port
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // GRPC Server
    let grpc_port = std::env::var("DOCKER_PORT_GRPC")
        .unwrap_or_else(|_| "50051".to_string())
        .parse::<u16>()
        .unwrap_or(50051);

    let full_grpc_addr = format!("[::]:{}", grpc_port).parse()?;

    let (mut health_reporter, health_service) = tonic_health::server::health_reporter();
    let imp = SvcTelemetryImpl::default();
    health_reporter
        .set_serving::<SvcTelemetryRpcServer<SvcTelemetryImpl>>()
        .await;

    //start server
    println!("Starting gRPC server at: {}", full_grpc_addr);
    Server::builder()
        .add_service(health_service)
        .add_service(SvcTelemetryRpcServer::new(imp))
        .serve(full_grpc_addr)
        .await?;
    println!("gRPC server running at: {}", full_grpc_addr);

    Ok(())
}
