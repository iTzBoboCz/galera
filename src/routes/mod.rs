use std::sync::Arc;
use crate::auth::login::{UserLogin, UserInfo, LoginResponse};
use crate::auth::token::{Claims, ClaimsEncoded};
use crate::db::{self, users::get_user_by_id};
use crate::directories::Directories;
use crate::models::NewUser;
use axum::Extension;
use axum::extract::State;
use axum::{Json, http::StatusCode};
use axum_extra::routing::TypedPath;
use tracing::{error, info};
use crate::{AppState, ConnectionPool, scan};
use serde::{Deserialize, Serialize};

pub mod oidc;
pub mod media;
pub mod albums;

// #[openapi]
// #[get("/")]
// pub async fn index() -> &'static str {
//   "Hello, world!"
// }

#[derive(TypedPath)]
#[typed_path("/health")]
pub struct Health;

pub async fn health(_: Health) -> StatusCode {
  StatusCode::OK
}

#[derive(TypedPath)]
#[typed_path("/user")]
pub struct UserRoute;

/// Creates a new user
pub async fn create_user(
  _: UserRoute,
  State(AppState { pool,.. }): State<AppState>,
  Json(user): Json<NewUser>,
) -> Result<StatusCode, StatusCode> {
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
  tags = ["auth:public"],
  request_body = UserLogin,
  responses(
    (status = 200, description = "Login successful", body = LoginResponse),
    (status = 409, description = "Invalid credentials or user conflict"),
    (status = 400, description = "Malformed request")
  )
)]
pub async fn login(
  _: LoginRoute,
  State(AppState { pool,.. }): State<AppState>,
  Json(user_login): Json<UserLogin>,
) -> Result<Json<LoginResponse>, StatusCode> {
  let token_option = user_login.hash_password().login(pool.clone()).await;
  if token_option.is_none() { return Err(StatusCode::CONFLICT); }

  let token = token_option.unwrap();

  issue_login_response(pool, token).await
}


async fn issue_login_response(pool: ConnectionPool, token: Claims) -> Result<Json<LoginResponse>, StatusCode> {
  let user_info = get_user_by_id(pool.get().await.unwrap(), token.user_id).await;
  if user_info.is_none() { return Err(StatusCode::INTERNAL_SERVER_ERROR) }

  let encoded = token.encode();
  if encoded.is_err() { return Err(StatusCode::INTERNAL_SERVER_ERROR) }

  Ok(
    Json(
      LoginResponse::new(
        encoded.unwrap(),
        UserInfo::from(user_info.unwrap())
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
pub async fn scan_media(
  _: ScanMediaRoute,
  State(AppState { pool,.. }): State<AppState>,
  Extension(claims): Extension<Arc<Claims>>
) -> Result<StatusCode, StatusCode> {
  let Some(directories) = Directories::new() else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  let Some(xdg_data) = directories.gallery().to_owned() else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  // this thread will run until scanning is complete
  tokio::spawn(scan::scan_root(pool, xdg_data, claims.user_id));

  Ok(StatusCode::OK)
}

// #[derive(JsonSchema)
#[derive(Serialize, Deserialize)]
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
pub async fn system_info_public(
  _: SystemInfoPublicRoute,
) -> Json<SystemInfoPublic> {
  Json(SystemInfoPublic::new())
}
