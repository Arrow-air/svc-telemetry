//! Temporary means of equating a request signature to an aircraft identifier.
//!
//! Remote ID message types are not guaranteed to contain the aircraft
//!  identifier (e.g. the basic message type does, location does not).
//!
//! The aircraft will "login" providing its identifier, and will be given a
//!  JWT in return. This JWT will be used to authenticate future requests
//!  and will be used to identify the aircraft, so that all remote id
//!  can be stored with the correct identifier.
//!
//! In the future, the login process will be replaced with a more secure
//!  method of authentication where the aircraft cannot be spoofed. This
//!  may be a PKI certificate that our network (as a certificate authority)
//!  issues to the device

use axum::{
    body::Bytes,
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
    Json,
};
use chrono::{Duration, Utc};
use hyper::Request;
use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};

use axum_extra::extract::cookie::CookieJar;
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};

/// JWT Encryption Type
const JWT_ENCRYPTION_TYPE: Algorithm = Algorithm::HS256;

/// TODO(R5): This is a temporary solution, replace with PKI certificates
pub static JWT_SECRET: OnceCell<String> = OnceCell::new();

/// JWT Expiration time in seconds
const JWT_EXPIRE_SECONDS: i64 = 360; // TODO(R5): To configuration file

/// Error Response
#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    /// Status
    status: String,

    /// Message
    message: String,
}

/// JWT Information
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claim {
    /// Subject
    pub sub: String,

    /// Issued at time in seconds
    pub iat: usize,

    /// Expiration time in seconds
    pub exp: usize,
}

impl Claim {
    /// Create and encode a JWT token
    pub fn create(sub: String) -> Result<String, StatusCode> {
        let header = Header::new(JWT_ENCRYPTION_TYPE);
        let iat = Utc::now().timestamp();
        let Ok(iat) = <usize>::try_from(iat) else {
            rest_error!("(Claim::create) could not convert IAT timestamp {iat} to usize.");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        };

        let Some(delta) = Duration::try_seconds(JWT_EXPIRE_SECONDS) else {
            rest_error!(
                "(Claim::create) could not create duration from {JWT_EXPIRE_SECONDS} seconds."
            );
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        };

        let exp = (Utc::now() + delta).timestamp();
        let Ok(exp) = <usize>::try_from(exp) else {
            rest_error!("(Claim::create) could not convert EXP timestamp {exp} to usize.");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        };

        let claims = Claim { sub, iat, exp };

        let Some(jwt_secret) = JWT_SECRET.get() else {
            rest_error!("(Claim::create) JWT_SECRET not set.");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        };

        let key = EncodingKey::from_secret(jwt_secret.as_bytes());
        encode(&header, &claims, &key).map_err(|e| {
            rest_error!("(Claim::create) could not encode JWT: {e}");
            StatusCode::INTERNAL_SERVER_ERROR
        })
    }

    /// Decode a JWT token
    pub fn decode(token: String) -> Result<Claim, StatusCode> {
        let Some(jwt_secret) = JWT_SECRET.get() else {
            rest_error!("(Claim::create) JWT_SECRET not set.");
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        };

        let key = DecodingKey::from_secret(jwt_secret.as_bytes());
        decode(&token, &key, &Validation::default())
            .map(|data| data.claims)
            .map_err(|e| {
                rest_error!("(Claim::decode) could not decode JWT: {e}");
                StatusCode::UNAUTHORIZED
            })
    }
}

/// Get bearer token from request
pub fn get_token_from_cookie_jar<B>(
    req: &Request<B>,
    cookie_jar: &CookieJar,
) -> Result<String, (StatusCode, Json<ErrorResponse>)>
where
    B: std::fmt::Debug,
{
    rest_info!("(get_token_from_cookie_jar) getting token from cookie jar.");
    if let Some(cookie) = cookie_jar.get("token") {
        return Ok(cookie.value().to_string());
    }

    // rest_debug!("(get_token_from_cookie_jar) request: {:?}", req);
    // rest_debug!("(get_token_from_cookie_jar) request headers: {:?}", req.headers());
    let Some(header) = req.headers().get(header::AUTHORIZATION) else {
        let message = "could not get authorization header.".to_string();
        rest_warn!("(get_token_from_cookie_jar) {message}");
        let json_error = ErrorResponse {
            status: "fail".to_string(),
            message,
        };

        return Err((StatusCode::UNAUTHORIZED, Json(json_error)));
    };

    let Some(auth_value) = header.to_str().ok() else {
        let message = "could not parse authorization header.".to_string();
        rest_warn!("(get_token_from_cookie_jar) {message}");
        let json_error = ErrorResponse {
            status: "fail".to_string(),
            message,
        };

        return Err((StatusCode::UNAUTHORIZED, Json(json_error)));
    };

    if let Some(substring) = auth_value.strip_prefix("Bearer ") {
        // rest_debug!("(get_token_from_cookie_jar) request token: {substring}");
        return Ok(substring.to_owned());
    }

    let message = "You are not logged in, please provide token.".to_string();
    rest_warn!("(get_token_from_cookie_jar) {message}");
    let json_error = ErrorResponse {
        status: "fail".to_string(),
        message,
    };

    Err((StatusCode::UNAUTHORIZED, Json(json_error)))
}

/// Authenticate a request with a JWT
pub async fn auth<B>(
    cookie_jar: CookieJar,
    mut req: Request<B>,
    next: Next<B>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)>
where
    B: std::fmt::Debug,
{
    rest_info!("(auth) authenticating request.");
    let token = get_token_from_cookie_jar(&req, &cookie_jar)?;

    // rest_debug!("(auth) request token: {token}");
    let claim = Claim::decode(token).map_err(|e| {
        rest_warn!("(auth) could not decode token: {e}");
        let json_error = ErrorResponse {
            status: "fail".to_string(),
            message: "Invalid token".to_string(),
        };
        (StatusCode::UNAUTHORIZED, Json(json_error))
    })?;

    rest_debug!("(auth) request claim: {:?}", claim);

    req.extensions_mut().insert(claim);
    Ok(next.run(req).await)
}

/// Remote ID Login
#[utoipa::path(
    get,
    path = "/telemetry/login",
    tag = "svc-telemetry",
    request_body = String, // identifier TODO(R5)
    responses(
        (status = 200, description = "Login successful, token returned."),
        (status = 400, description = "Bad request."),
        (status = 500, description = "Something went wrong."),
        (status = 503, description = "Dependencies of svc-telemetry were down."),
    )
)]
pub async fn login(identifier: Bytes) -> Result<Json<String>, StatusCode> {
    let identifier = String::from_utf8(identifier.to_vec()).map_err(|_| StatusCode::BAD_REQUEST)?;
    if identifier.is_empty() {
        rest_warn!("(login) empty identifier, failing login request.");
        return Err(StatusCode::BAD_REQUEST);
    }

    let token = Claim::create(identifier)?;
    Ok(Json(token))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{middleware, routing::post, Extension, Router};
    use hyper::{Method, Request};
    use tower::ServiceExt;

    #[tokio::test]
    async fn middleware_runs() {
        async fn handler(Extension(claim): Extension<Claim>) {
            crate::get_log_handle().await;
            ut_info!("(middleware_runs): {:#?}", claim);
            serde_json::to_string(&claim).unwrap();
        }

        JWT_SECRET.set("test".to_string()).unwrap();

        let router: Router = Router::new()
            .route("/", post(handler))
            .route_layer(middleware::from_fn(auth));

        let token = Claim::create("test".to_string()).unwrap();
        let req = Request::builder()
            .uri("/")
            .method(Method::POST)
            .header("content-type", "application/octet-stream")
            .header("Authorization", format!("Bearer {token}"))
            .body(Bytes::from(vec![0x82]).into())
            .unwrap();

        router.oneshot(req).await.unwrap();
    }
}
