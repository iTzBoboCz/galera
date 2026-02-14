use std::sync::Arc;
use crate::auth::login::{UserLogin, UserInfo, LoginResponse};
use crate::auth::token::Claims;
use crate::cookies::{build_refresh_cookie, clear_refresh_cookie, read_refresh_token};
use crate::db::tokens::{delete_obsolete_access_tokens, delete_session_by_refresh_token};
use crate::db::{self, users::get_user_by_id};
use crate::directories::Directories;
use crate::models::{NewUser, User, UserInsert};
use crate::openapi::tags::{AUTH, AUTH_PROTECTED, AUTH_PUBLIC, OTHER};
use axum::Extension;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::{Json, http::StatusCode};
use axum_extra::extract::CookieJar;
use axum_extra::routing::TypedPath;
use tracing::info;
use utoipa::ToSchema;
use uuid::Uuid;
use crate::{AppState, scan};
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
  request_body = UserInsert,
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
  Json(user): Json<UserInsert>,
) -> Result<StatusCode, StatusCode> {
  if auth_policy.disable_local_auth || auth_policy.disable_local_signups {
    return Err(StatusCode::SERVICE_UNAVAILABLE);
  }

  if !user.check() { return Err(StatusCode::UNPROCESSABLE_ENTITY) }

  // TODO: investigate passing pool vs connection as parameter
  if !db::users::is_user_unique(pool.get().await.unwrap(), user.clone()).await { return Err(StatusCode::CONFLICT); };

  let new_user = NewUser::from(user.hash_password());
  let result = db::users::insert_user(pool.get().await.unwrap(), new_user.clone()).await;
  if result == 0 { return Err(StatusCode::INTERNAL_SERVER_ERROR) }

  info!("A new user was created with name {}", new_user.username);
  Ok(StatusCode::OK)
}

#[derive(TypedPath)]
#[typed_path("/auth/login")]
pub struct LoginRoute;

/// You must provide either a username or an email together with a password.
#[utoipa::path(
  post,
  path = "/auth/login",
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
  jar: CookieJar,
  headers: HeaderMap,
  Json(user_login): Json<UserLogin>,
) -> Result<(CookieJar, Json<LoginResponse>), StatusCode> {
  if auth_policy.disable_local_auth {
    return Err(StatusCode::SERVICE_UNAVAILABLE);
  }

  let refresh_token = Uuid::new_v4().to_string();
  let Some(token) = user_login.hash_password().login(pool.clone(), refresh_token.clone()).await else {
    return Err(StatusCode::CONFLICT);
  };

  let jar = jar.add(build_refresh_cookie(refresh_token, &headers));

  let Some(user) = get_user_by_id(pool.get().await.unwrap(), token.user_id).await else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };
  issue_login_response(user, token, jar).await
}

async fn issue_login_response(user: User, token: Claims, jar: CookieJar) -> Result<(CookieJar, Json<LoginResponse>), StatusCode> {

  let Ok(encoded) = token.encode() else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  Ok(
    (
      jar,
      Json(
        LoginResponse::new(
          encoded,
          UserInfo::from(user)
        )
      )
    )
  )
}

#[derive(TypedPath)]
#[typed_path("/auth/logout")]
pub struct LogoutRoute;

/// Invalidates the session.
#[utoipa::path(
  post,
  path = "/auth/logout",
  tags = [ AUTH, AUTH_PUBLIC ],
  responses(
    (status = 204, description = "Logout succesful"),
  )
)]
pub async fn logout(
  _: LogoutRoute,
  State(AppState { pool,.. }): State<AppState>,
  headers: HeaderMap,
  jar: CookieJar,
) -> (CookieJar, StatusCode) {
  if let Some(refresh_token) = read_refresh_token(&jar) {
    match delete_session_by_refresh_token(pool.get().await.unwrap(), refresh_token).await {
      Ok(true) => {} // deleted
      Ok(false) => tracing::debug!("logout: no session found"),
      Err(e) => tracing::warn!("logout: failed to invalidate session: {e}"),
    }
  }

  (jar.add(clear_refresh_cookie(&headers)), StatusCode::NO_CONTENT)
}

#[derive(TypedPath)]
#[typed_path("/auth/refresh")]
pub struct LoginRefreshRoute;

/// Issues a new access token when a valid refresh token is attached
#[utoipa::path(
  post,
  path = "/auth/refresh",
  tags = [ AUTH, AUTH_PUBLIC ],
  responses(
    (status = 200, description = "Token refreshed", body = LoginResponse),
    (status = 401, description = "Unauthorized (missing/invalid/expired refresh_token cookie)"),
    (status = 500, description = "Internal server error")
  )
)]
pub async fn refresh_token(
  _: LoginRefreshRoute,
  State(AppState { pool,.. }): State<AppState>,
  jar: CookieJar,
) -> Result<(CookieJar, Json<LoginResponse>), StatusCode> {
  let refresh_token = read_refresh_token(&jar)
    .ok_or(StatusCode::UNAUTHORIZED)?;

  let Some((refresh_token_id, user_id)) =
    db::tokens::select_refresh_token_session(pool.get().await.unwrap(), refresh_token.clone())
      .await
      .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
  else {
    return Err(StatusCode::UNAUTHORIZED);
  };

  // refresh token is expired
  if Claims::is_refresh_token_expired(pool.get().await.unwrap(), refresh_token.clone()).await { return Err(StatusCode::UNAUTHORIZED); }

  let Some(user) = get_user_by_id(pool.get().await.unwrap(), user_id).await else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  let user_uuid = user.uuid.clone();
  let new_token = Claims::new(user.id, user_uuid);

  delete_obsolete_access_tokens(pool.get().await.unwrap(), refresh_token_id)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

  new_token.add_access_token_to_db(pool, refresh_token_id)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

  issue_login_response(user, new_token, jar).await
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
