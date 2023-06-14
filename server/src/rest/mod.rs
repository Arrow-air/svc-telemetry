//! REST
//! provides server implementations for REST API

#[macro_use]
pub mod macros;
pub mod api;
pub mod server;

use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(
    paths(
        api::mavlink_adsb,
        api::adsb
    ),
    tags(
        (name = "svc-telemetry", description = "svc-telemetry REST API.")
    )
)]
struct ApiDoc;

/// Create OpenAPI3 Specification File
pub fn generate_openapi_spec(target: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = ApiDoc::openapi()
        .to_pretty_json()
        .expect("(ERROR) unable to write openapi specification to json.");

    std::fs::write(target, output).expect("(ERROR) unable to write json string to file.");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_openapi_spec_generation() {
        assert!(generate_openapi_spec("/tmp/generate_openapi_spec.out").is_ok());
    }
}
