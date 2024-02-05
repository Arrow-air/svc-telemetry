//! Rest server implementation

use super::api;
use crate::amqp::init_mq;
use crate::cache::pool::RedisPool;
use crate::cache::RedisPools;
use crate::grpc::client::GrpcClients;
use crate::shutdown_signal;
use crate::Config;
use axum::{
    error_handling::HandleErrorLayer,
    extract::Extension,
    http::{HeaderValue, StatusCode},
    routing::{get, post},
    BoxError, Router,
};
use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use svc_gis_client_grpc::client::{AircraftId, AircraftPosition, AircraftVelocity};
use tower::{
    buffer::BufferLayer,
    limit::{ConcurrencyLimitLayer, RateLimitLayer},
    ServiceBuilder,
};
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// Starts the REST API server for this microservice
///
/// # Example:
/// ```
/// use svc_telemetry::rest::server::rest_server;
/// use svc_telemetry::grpc::client::GrpcClients;
/// use svc_gis_client_grpc::client::{AircraftPosition, AircraftVelocity, AircraftId};
/// use svc_telemetry::Config;
/// use std::collections::VecDeque;
/// use std::sync::{Arc, Mutex};
/// async fn example() -> Result<(), tokio::task::JoinError> {
///     let config = Config::default();
///     let id_ring = Arc::new(Mutex::new(VecDeque::<AircraftId>::with_capacity(10)));
///     let position_ring = Arc::new(Mutex::new(VecDeque::<AircraftPosition>::with_capacity(10)));
///     let velocity_ring = Arc::new(Mutex::new(VecDeque::<AircraftVelocity>::with_capacity(10)));
///     let grpc_clients = GrpcClients::default(config.clone());
///     tokio::spawn(rest_server(config, grpc_clients, id_ring, position_ring, velocity_ring, None)).await;
///     Ok(())
/// }
/// ```
#[cfg(not(tarpaulin_include))]
// no_coverage: Needs running backends to work.
// Will be tested in integration tests.
pub async fn rest_server(
    config: Config,
    grpc_clients: GrpcClients,
    id_ring: Arc<Mutex<VecDeque<AircraftId>>>,
    position_ring: Arc<Mutex<VecDeque<AircraftPosition>>>,
    velocity_ring: Arc<Mutex<VecDeque<AircraftVelocity>>>,
    shutdown_rx: Option<tokio::sync::oneshot::Receiver<()>>,
) -> Result<(), ()> {
    rest_info!("(rest_server) entry.");
    let rest_port = config.docker_port_rest;
    let full_rest_addr: SocketAddr = match format!("[::]:{}", rest_port).parse() {
        Ok(addr) => addr,
        Err(e) => {
            rest_error!("(rest_server) invalid address: {:?}, exiting.", e);
            return Err(());
        }
    };

    let cors_allowed_origin = match config.rest_cors_allowed_origin.parse::<HeaderValue>() {
        Ok(url) => url,
        Err(e) => {
            rest_error!(
                "(rest_server) invalid cors_allowed_origin address: {:?}, exiting.",
                e
            );
            return Err(());
        }
    };

    // Rate limiting
    let rate_limit = config.rest_request_limit_per_second as u64;
    let concurrency_limit = config.rest_concurrency_limit_per_service as usize;
    let limit_middleware = ServiceBuilder::new()
        .layer(TraceLayer::new_for_http())
        .layer(HandleErrorLayer::new(|e: BoxError| async move {
            rest_warn!("(rest_server) too many requests: {}", e);
            (
                StatusCode::TOO_MANY_REQUESTS,
                "(rest_server) too many requests.".to_string(),
            )
        }))
        .layer(BufferLayer::new(100))
        .layer(ConcurrencyLimitLayer::new(concurrency_limit))
        .layer(RateLimitLayer::new(
            rate_limit,
            std::time::Duration::from_secs(1),
        ));

    //
    // Extensions
    //

    // Redis Pools
    let pools = RedisPools {
        mavlink: RedisPool::new(config.clone(), "tlm:mav").await?,
        adsb: RedisPool::new(config.clone(), "tlm:adsb").await?,
        netrid: RedisPool::new(config.clone(), "tlm:netrid").await?,
    };

    // RabbitMQ Channel
    let mq_channel = init_mq(config.clone())
        .await
        .map_err(|_| rest_error!("(rest_server) could not create RabbitMQ Channel."));

    // TODO(R5): Replace with PKI certificates
    match crate::rest::api::jwt::JWT_SECRET.set(config.jwt_secret.clone()) {
        Err(e) => {
            rest_error!("(rest_server) could not set JWT_SECRET: {}", e);
            return Err(());
        }
        _ => {
            rest_info!("(rest_server) set JWT_SECRET.");
        }
    }

    //
    // Create Server
    //
    let app = Router::new()
        .route("/health", get(api::health::health_check))
        .route("/telemetry/login", get(crate::rest::api::jwt::login))
        .route(
            "/telemetry/netrid",
            post(api::netrid::network_remote_id)
                .route_layer(axum::middleware::from_fn(crate::rest::api::jwt::auth)),
        )
        .route("/telemetry/mavlink/adsb", post(api::mavlink::mavlink_adsb))
        .route("/telemetry/adsb", post(api::adsb::adsb))
        .layer(
            CorsLayer::new()
                .allow_origin(cors_allowed_origin)
                .allow_headers(Any)
                .allow_methods(Any),
        )
        .layer(limit_middleware)
        .layer(Extension(pools))
        .layer(Extension(mq_channel))
        .layer(Extension(grpc_clients))
        .layer(Extension(id_ring))
        .layer(Extension(position_ring))
        .layer(Extension(velocity_ring));

    match axum::Server::bind(&full_rest_addr)
        .serve(app.into_make_service())
        .with_graceful_shutdown(shutdown_signal("rest", shutdown_rx))
        .await
    {
        Ok(_) => {
            rest_info!("(rest_server) hosted at: {}.", full_rest_addr);
            Ok(())
        }
        Err(e) => {
            rest_error!("(rest_server) could not start server: {}", e);
            Err(())
        }
    }
}
