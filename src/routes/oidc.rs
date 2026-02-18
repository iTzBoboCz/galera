use std::time::Instant;
use crate::auth::token::Claims;
use crate::config::get_frontend_callback_url;
use crate::cookies::build_refresh_cookie;
use crate::db::oidc::insert_oidc_user;
use crate::db::users::get_user_by_id;
use crate::models::{DataJsonOidc, SessionOriginMethod, User};
use crate::openapi::tags::{AUTH, AUTH_PUBLIC, OIDC, OTHER};
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect};
use axum::{Json, http::StatusCode};
use axum_extra::extract::CookieJar;
use axum_extra::routing::TypedPath;
use openidconnect::core::CoreAuthenticationFlow;
use openidconnect::{AuthorizationCode, CsrfToken, Nonce, Scope, TokenResponse};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, warn};
use utoipa::ToSchema;
use uuid::Uuid;
use crate::{AppState, ConnectionPool, OidcState, db};

#[derive(TypedPath, Deserialize)]
#[typed_path("/auth/oidc/{provider}/login")]
pub struct OidcLogin {
  pub provider: String,
}

#[derive(Deserialize)]
pub struct OidcLoginQuery {
  pub redirect: Option<String>
}

#[utoipa::path(
  get,
  path = "/auth/oidc/{provider}/login",
  params(
    ("provider" = String, Path, description = "OIDC provider key"),
    ("redirect" = Option<String>, Query, description = "Frontend relative path to redirect to after login")
  ),
  tags = [ AUTH, OIDC, AUTH_PUBLIC ],
  responses(
    (status = 302, description = "Redirect to OIDC provider"),
    (status = 404, description = "OIDC provider not found"),
    (status = 503, description = "OIDC is disabled")
  )
)]
pub async fn oidc_login(
  OidcLogin { provider }: OidcLogin,
  Query(OidcLoginQuery { redirect: unsanitized_redirect }): Query<OidcLoginQuery>,
  State(state): State<AppState>,
) -> impl IntoResponse {
    let oidc = match &state.oidc {
    OidcState::Disabled => {
      return (StatusCode::SERVICE_UNAVAILABLE, "OIDC is disabled").into_response();
    }
    OidcState::Enabled(enabled) => enabled,
  };

  let prov = match oidc.oidc_providers.get(&provider) {
    Some(p) => p,
    None => return (StatusCode::NOT_FOUND, "Unknown OIDC provider").into_response(),
  };

  // If you store OidcProvider { client, ... }, use: let client = &prov.client;
  // If you store raw clients, use: let client = &*prov;
  let client = &prov.client;

  let (auth_url, csrf_token, nonce) = client
    .authorize_url(
      CoreAuthenticationFlow::AuthorizationCode,
      CsrfToken::new_random,
      Nonce::new_random,
    )
    .add_scope(Scope::new("openid".into()))
    .add_scope(Scope::new("profile".into()))
    .add_scope(Scope::new("email".into()))
    .url();

    let redirect = crate::oidc::sanitize_frontend_redirect(unsanitized_redirect);

  // Store state -> nonce + provider for callback validation
  oidc.login_states.insert(
    csrf_token.secret().to_owned(),
    crate::oidc::PendingLogin {
      provider: provider.clone(),
      nonce,
      created_at: Instant::now(),
      redirect
    },
  );

  Redirect::temporary(auth_url.as_str()).into_response()
}


#[derive(TypedPath, Deserialize)]
#[typed_path("/auth/oidc/{provider}/callback")]
pub struct OidcCallback {
  pub provider: String,
}

// 10 minutes
const LOGIN_STATE_TTL_SECS: u64 = 10 * 60;

#[derive(Deserialize)]
pub struct OidcCallbackQuery {
  code: String,
  state: String
}

#[utoipa::path(
  get,
  path = "/auth/oidc/{provider}/callback",
  tags = [ AUTH, OIDC, AUTH_PUBLIC ],

  params(
    ("provider" = String, Path, description = "OIDC provider key"),
    ("code" = String, Query, description = "Authorization code"),
    ("state" = String, Query, description = "CSRF state")
  ),
  responses(
    (status = 302, description = "Redirect to frontend callback"),
    (status = 400, description = "Bad request"),
    (status = 401, description = "Authentication failed"),
    (status = 404, description = "Provider not found"),
    (status = 500, description = "Internal server error"),
    (status = 503, description = "OIDC is disabled")
  )
)]
pub async fn oidc_callback(
  OidcCallback { provider }: OidcCallback,
  Query(q): Query<OidcCallbackQuery>,
  State(state): State<AppState>,
  headers: HeaderMap,
  jar: CookieJar
) -> Result<(CookieJar, Redirect), (StatusCode, &'static str)> {
    // 0) Hard-disable the endpoint if OIDC is disabled
  let oidc = match &state.oidc {
    OidcState::Disabled => {
      return Err((StatusCode::SERVICE_UNAVAILABLE, "OIDC is disabled"));
    }
    OidcState::Enabled(enabled) => enabled,
  };

  // Validate and consume CSRF "state"
  let pending = match oidc.login_states.remove(&q.state) {
    Some((_, p)) => p,
    None => return Err((StatusCode::BAD_REQUEST, "Invalid/expired state")),
  };

  // Check if state = csrf_state per docs
  if pending.provider != provider {
    return Err((StatusCode::BAD_REQUEST, "Provider mismatch"));
  }

  if pending.created_at.elapsed().as_secs() > LOGIN_STATE_TTL_SECS {
    return Err((StatusCode::BAD_REQUEST, "Login expired"));
  }

  // 2) Get provider client
  let prov = match oidc.oidc_providers.get(&provider) {
    Some(p) => p,
    None => return Err((StatusCode::NOT_FOUND, "Unknown OIDC provider")),
  };

  let client = &prov.client;

  // 3) Exchange code -> tokens (client_secret verified here by IdP)
  let token_request = match client.exchange_code(AuthorizationCode::new(q.code)) {
    Ok(req) => req,
    Err(e) => {
      warn!("token endpoint not set / exchange_code failed: {e}");
      return Err((StatusCode::BAD_REQUEST, "OIDC token endpoint not available"));
    }
  };

  let token_response = match token_request
    .request_async(&oidc.http_client)
    .await
  {
    Ok(t) => t,
    Err(e) => {
      warn!("token exchange failed: {e}");
      return Err((StatusCode::UNAUTHORIZED, "Token exchange failed"));
    }
  };

  // 4) Verify ID token signature + nonce
  let id_token = match token_response.id_token() {
    Some(t) => t,
    None => return Err((StatusCode::UNAUTHORIZED, "Missing id_token")),
  };

  let claims = match id_token.claims(&client.id_token_verifier(), &pending.nonce) {
    Ok(c) => c,
    Err(e) => {
      warn!("id_token verification failed: {e}");
      return Err((StatusCode::UNAUTHORIZED, "Invalid id_token"));
    }
  };

  let sub = claims.subject().as_str().to_owned();
  let Some(email) = claims.email().map(|e| e.as_str().to_owned()) else {
    return Err((StatusCode::BAD_REQUEST, "Missing email"));
  };

  debug!("OIDC login ok provider={} sub={} email={:?}", provider, sub, email);

  // 5) Create session origin
  let session_origin = SessionOriginMethod::OIDC {
    provider_key: provider.clone(), subject: sub.clone(), data_json: Some(DataJsonOidc {
      id_token: id_token.to_string(),
      sid: None
    })
  };

  // 6) Find existing identity by (provider, sub)
  match db::oidc::get_user_by_oidc_subject(state.pool.get().await.unwrap(), provider.clone(), sub.clone()).await {
    Ok(Some(oidc_identity)) => {
      let Some(user) = get_user_by_id(state.pool.get().await.unwrap(), oidc_identity.user_id).await else {
        error!("DB error selecting user by user_id after succesful oidc_identity select");
        return Err((StatusCode::INTERNAL_SERVER_ERROR, ""));
      };
      let claims = Claims::new(user.id, user.uuid);
      return issue_oidc_login_response(state.pool, headers, claims, jar, pending.redirect, session_origin).await;
    }

    // Continue to create a user
    Ok(None) => {}

    Err(e) => {
      error!("DB error selecting oidc identity: {e}");
      return Err((StatusCode::INTERNAL_SERVER_ERROR, ""));
    }
  }

  // 6b) If enabled, try to link an existing LOCAL user by email
  let oidc_link_existing_by_email = std::env::var("OIDC_LINK_EXISTING_BY_EMAIL")
    .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1"))
    .unwrap_or(false);

  if oidc_link_existing_by_email {
    // Look up local user by email
    match db::users::get_user_by_email(state.pool.get().await.unwrap(), email.clone()).await {
      Ok(Some(existing)) => {
        let email_trusted = claims.email_verified().unwrap_or(false);

        if !email_trusted {
          warn!(
            "Refusing to link OIDC identity to existing user by email because email_verified is false for email={:?} (provider={})",
            email, provider
          );
          return Err((StatusCode::UNAUTHORIZED, "Email not verified"));
        }

        // Link identity: (provider, sub) -> existing user id
        match db::oidc::insert_oidc_identity_link(
          state.pool.get().await.unwrap(),
          existing.id,
          provider.clone(),
          sub.clone(),
        ).await {
          Ok(()) => {
            return issue_oidc_login_response(state.pool, headers, Claims::new(existing.id, existing.uuid), jar, pending.redirect, session_origin).await;
          }
          Err(e) => {
            error!("DB error inserting oidc identity link: {e}");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, ""));
          }
        }
      }
      // no existing user by email -> continue to signup gate
      Ok(None) => {}
      Err(e) => {
        error!("DB error selecting user by email: {e}");
        return Err((StatusCode::INTERNAL_SERVER_ERROR, ""));
      }
    }
  }

  // 7) Not found → signup gate
  if !prov.config.allow_signup {
    return Err((StatusCode::UNAUTHORIZED, "Signups disabled"));
  }

  // 8) Create new local user (OIDC-only) + link identity
  let Ok(user_id) = insert_oidc_user(state.pool.get().await.unwrap(), provider.clone(), sub.clone(), email).await else {
    debug!("Created a new OIDC-only user - IdP provider: {}, IdP sub: {}", provider, sub);
    return Err((StatusCode::INTERNAL_SERVER_ERROR, "Can't create new oidc-only user"));
  };

  let Some(User { uuid,.. }) = get_user_by_id(state.pool.get().await.unwrap(), user_id).await else {
    error!("DB error selecting user by user_id");
    return Err((StatusCode::INTERNAL_SERVER_ERROR, ""));
  };

  // 9) Issue normal JWT login response
  issue_oidc_login_response(state.pool, headers, Claims::new(user_id, uuid), jar, pending.redirect, session_origin).await
}

pub async fn issue_oidc_login_response(
  pool: ConnectionPool,
  headers: HeaderMap,
  claims: Claims,
  jar: CookieJar,
  redirect: Option<String>,
  session_origin: SessionOriginMethod,
) -> Result<(CookieJar, Redirect), (StatusCode, &'static str)> {
  let refresh_token = Uuid::new_v4().to_string();


  if let Err(e) = claims.add_session_tokens_to_db(pool.clone(), refresh_token.clone(), session_origin).await {
    error!("Failed to insert session tokens during OIDC callback: {e}");
    return Err((StatusCode::INTERNAL_SERVER_ERROR, "".into()));
  }

  let jar = jar.add(build_refresh_cookie(refresh_token, &headers));

  let Some(mut frontend_callback_url) = get_frontend_callback_url() else {
    error!("FRONTEND_URL not set or invalid (cannot redirect after OIDC login)");
    return Err((StatusCode::INTERNAL_SERVER_ERROR, "".into()));
  };

    if let Some(r) = redirect.as_deref() {
      frontend_callback_url
        .query_pairs_mut()
        .append_pair("redirect", r);
    }

  Ok((jar, Redirect::to(frontend_callback_url.as_str())))
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct ServerConfigResponse {
  auth: AuthConfig,
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct AuthConfig {
    pub oidc: Vec<OidcProviderPublic>,
    pub policy: AuthPolicyPublic
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct OidcProviderPublic {
    pub key: String,
    pub display_name: String,
    pub login_url: String,
}

#[derive(Clone, Serialize, Deserialize, ToSchema)]
pub struct AuthPolicyPublic {
  pub disable_local_signups: bool,
  pub disable_local_auth: bool,
}

#[derive(TypedPath)]
#[typed_path("/public/config")]
pub struct ServerConfig;

/// Returns server configuration
#[utoipa::path(
  get,
  path = "/public/config",
  tags = [ OIDC, OTHER, AUTH_PUBLIC ],
  responses(
    (status = 200, description = "Server config", body = ServerConfigResponse)
  )
)]
pub async fn get_server_config(
  _: ServerConfig,
  State(state): State<AppState>,
) -> Json<ServerConfigResponse> {
  let oidc = match &state.oidc {
    OidcState::Disabled => Vec::new(),
    OidcState::Enabled(enabled) => {
      enabled.oidc_providers
        .iter()
        .map(|entry| {
          let provider = entry.value();

          let key = provider.key.clone();

          // use OIDC_PROVIDER_KEY when OIDC_DISPLAY_NAME isn't available
          let display_name = provider.display_name.clone().unwrap_or_else(|| key.clone());

          OidcProviderPublic {
            display_name,
            login_url: format!("/auth/oidc/{}/login", key),
            key,
          }
        })
        .collect::<Vec<_>>()
    }
  };

  Json(ServerConfigResponse {
    auth: AuthConfig { oidc, policy: state.auth_policy },
  })
}
