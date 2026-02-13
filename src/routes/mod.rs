use std::sync::Arc;
use crate::auth::login::{UserLogin, UserInfo, LoginResponse};
use crate::auth::token::{Claims, ClaimsEncoded};
use crate::db::{self, users::get_user_by_id};
use crate::directories::Directories;
use crate::models::NewUser;
use crate::openapi::tags::{AUTH, AUTH_PROTECTED, AUTH_PUBLIC, OTHER};
use axum::Extension;
use axum::extract::State;
use axum::{Json, http::StatusCode};
use axum_extra::routing::TypedPath;
use tracing::{info};
use utoipa::ToSchema;
use crate::{AppState, ConnectionPool, scan};
use serde::{Deserialize, Serialize};

pub mod oidc;
pub mod media;
pub mod albums;

#[derive(TypedPath)]
#[typed_path("/health")]
pub struct Health;

#[utoipa::path(
  get,
  path = "/health",
  tags = [ OTHER, AUTH_PUBLIC ],
  responses(
    (status = 200, description = "Health check passed")
  )
)]
pub async fn health(_: Health) -> StatusCode {
  StatusCode::OK
}

#[derive(TypedPath)]
#[typed_path("/user")]
pub struct UserRoute;

/// Creates a new user
#[utoipa::path(
  post,
  path = "/user",
  tags = [ AUTH, AUTH_PROTECTED ],
  request_body = NewUser,
  responses(
    (status = 200, description = "User created"),
    (status = 400, description = "Invalid JSON or wrong shape"),
    (status = 409, description = "User already exists"),
    (status = 422, description = "Invalid user data"),
    (status = 500, description = "Internal server error"),
    (status = 503, description = "Either local auth or signups are disabled")
  )
)]
pub async fn create_user(
  _: UserRoute,
  State(AppState { pool, auth_policy,..  }): State<AppState>,
  Json(user): Json<NewUser>,
) -> Result<StatusCode, StatusCode> {
  if auth_policy.disable_local_auth || auth_policy.disable_local_signups {
    return Err(StatusCode::SERVICE_UNAVAILABLE);
  }

  if !user.check() { return Err(StatusCode::UNPROCESSABLE_ENTITY) }

  // TODO: investigate passing pool vs connection as parameter
  if !db::users::is_user_unique(pool.get().await.unwrap(), user.clone()).await { return Err(StatusCode::CONFLICT); };

  let new_user = user.hash_password();
  let result = db::users::insert_user(pool.get().await.unwrap(), new_user.clone()).await;
  if result == 0 { return Err(StatusCode::INTERNAL_SERVER_ERROR) }

  info!("A new user was created with name {}", new_user.username);
  Ok(StatusCode::OK)
}

#[derive(TypedPath)]
#[typed_path("/login")]
pub struct LoginRoute;

/// You must provide either a username or an email together with a password.
#[utoipa::path(
  post,
  path = "/login",
  tags = [ AUTH, AUTH_PUBLIC ],
  request_body = UserLogin,
  responses(
    (status = 200, description = "Login successful", body = LoginResponse),
    (status = 400, description = "Invalid JSON or wrong shape"),
    (status = 409, description = "Invalid credentials or user conflict"),
    (status = 500, description = "Internal server error"),
    (status = 503, description = "Local auth is disabled")
  )
)]
pub async fn login(
  _: LoginRoute,
  State(AppState { pool, auth_policy,.. }): State<AppState>,
  Json(user_login): Json<UserLogin>,
) -> Result<Json<LoginResponse>, StatusCode> {
  if auth_policy.disable_local_auth {
    return Err(StatusCode::SERVICE_UNAVAILABLE);
  }

  let Some(token) = user_login.hash_password().login(pool.clone()).await else {
    return Err(StatusCode::CONFLICT);
  };

  issue_login_response(pool, token).await
}


async fn issue_login_response(pool: ConnectionPool, token: Claims) -> Result<Json<LoginResponse>, StatusCode> {
  let Some(user) = get_user_by_id(pool.get().await.unwrap(), token.user_id).await else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  let Ok(encoded) = token.encode() else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  Ok(
    Json(
      LoginResponse::new(
        encoded,
        UserInfo::from(user)
      )
    )
  )
}

#[derive(TypedPath)]
#[typed_path("/login/refresh")]
pub struct LoginRefreshRoute;

/// Refreshes sent token
// TODO: send token in header instead of body
// https://stackoverflow.com/a/53881397
#[utoipa::path(
  post,
  path = "/login/refresh",
  tags = [ AUTH, AUTH_PROTECTED ],
  request_body = ClaimsEncoded,
  responses(
    (status = 200, description = "Token refreshed", body = ClaimsEncoded),
    (status = 400, description = "Invalid JSON or wrong shape"),
    (status = 401, description = "Unauthorized"),
    (status = 500, description = "Internal server error")
  )
)]
pub async fn refresh_token(
  _: LoginRefreshRoute,
  State(AppState { pool,.. }): State<AppState>,
  Json(encoded_bearer_token): Json<ClaimsEncoded>,
) -> Result<Json<ClaimsEncoded>, StatusCode> {
  let decoded = encoded_bearer_token.clone().decode();
  let bearer_token: Claims;

  // access token is expired - most of the time (token needs to be refreshed because it is expired)
  if decoded.is_err() {
    let expired = match decoded.unwrap_err().kind() {
      jsonwebtoken::errors::ErrorKind::ExpiredSignature => true,
      _ => false
    };

    // the error is not expired token
    if !expired { return Err(StatusCode::UNAUTHORIZED) }

    let temp = encoded_bearer_token.decode_without_validation();

    // couldn't be decoded
    if temp.is_err() { return Err(StatusCode::UNAUTHORIZED) }


    bearer_token = temp.unwrap().claims
  } else {
    // access token is not yet expired
    bearer_token = decoded.unwrap().claims;
  }

  // refresh token is expired
  if bearer_token.is_refresh_token_expired(pool.get().await.unwrap()).await { return Err(StatusCode::UNAUTHORIZED); }

  let new_token = Claims::from_existing(&bearer_token);

  let Some(refresh_token_id) = db::tokens::select_refresh_token_id(pool.get().await.unwrap(), bearer_token.refresh_token()).await else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  Claims::delete_obsolete_access_tokens(pool.get().await.unwrap(), refresh_token_id).await;

  if new_token.add_access_token_to_db(pool, refresh_token_id).await.is_err() { return Err(StatusCode::INTERNAL_SERVER_ERROR); }

  let Ok(new_encoded_token) = new_token.encode() else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  Ok(Json(new_encoded_token))
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/scan_media")]
pub struct ScanMediaRoute;

/// Searches for new media
#[utoipa::path(
  get,
  path = "/scan_media",
  tags = [ OTHER, AUTH_PROTECTED ],
  security(("BearerAuth" = [])),
  responses(
    (status = 200, description = "Scan started"),
    (status = 401, description = "Unauthorized"),
    (status = 500, description = "Internal server error")
  )
)]
pub async fn scan_media(
  _: ScanMediaRoute,
  State(AppState { pool,.. }): State<AppState>,
  Extension(claims): Extension<Arc<Claims>>
) -> Result<StatusCode, StatusCode> {
  let Ok(gallery_dir) = Directories::get().gallery_dir() else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  // this thread will run until scanning is complete
  tokio::spawn(scan::scan_root(pool, gallery_dir, claims.user_id));

  Ok(StatusCode::OK)
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfoPublic {
  operating_system: String,
  server_name: String,
  server_version: String,
  system_architecture: String,
}

impl SystemInfoPublic {
  pub fn new() -> Self {
    Self {
      operating_system: std::env::consts::OS.to_string(),
      server_name: sys_info::hostname().unwrap_or(String::new()),
      server_version: env!("CARGO_PKG_VERSION").to_string(),
      system_architecture: std::env::consts::ARCH.to_string(),
    }
  }
}

#[derive(TypedPath)]
#[typed_path("/system/info/public")]
pub struct SystemInfoPublicRoute;

/// Returns the public system information.
#[utoipa::path(
  get,
  path = "/system/info/public",
  tags = [ OTHER, AUTH_PUBLIC ],
  responses(
    (status = 200, description = "System info", body = SystemInfoPublic)
  )
)]
pub async fn system_info_public(
  _: SystemInfoPublicRoute,
) -> Json<SystemInfoPublic> {
  Json(SystemInfoPublic::new())
}
