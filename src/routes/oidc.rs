use std::time::Instant;
use crate::auth::token::Claims;
use crate::auth::login::LoginResponse;
use crate::db::oidc::insert_oidc_user;
use crate::openapi::tags::{AUTH, AUTH_PUBLIC, OIDC, OTHER};
use crate::routes::issue_login_response;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect};
use axum::{Json, http::StatusCode};
use axum_extra::routing::TypedPath;
use openidconnect::core::CoreAuthenticationFlow;
use openidconnect::{AuthorizationCode, CsrfToken, Nonce, Scope, TokenResponse};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, warn};
use utoipa::ToSchema;
use crate::{AppState, OidcState, db};

#[derive(TypedPath, Deserialize)]
#[typed_path("/auth/oidc/{provider}/login")]
pub struct OidcLogin {
  pub provider: String,
}

#[utoipa::path(
  get,
  path = "/auth/oidc/{provider}/login",
  params(
    ("provider" = String, Path, description = "OIDC provider key")
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

  // Store state -> nonce + provider for callback validation
  oidc.login_states.insert(
    csrf_token.secret().to_owned(),
    crate::oidc::PendingLogin {
      provider: provider.clone(),
      nonce,
      created_at: Instant::now(),
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
    (status = 200, description = "Login successful", body = LoginResponse),
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
  State(state): State<AppState>
) -> impl IntoResponse {
    // 0) Hard-disable the endpoint if OIDC is disabled
  let oidc = match &state.oidc {
    OidcState::Disabled => {
      return (StatusCode::SERVICE_UNAVAILABLE, "OIDC is disabled").into_response();
    }
    OidcState::Enabled(enabled) => enabled,
  };

  // Validate and consume CSRF "state"
  let pending = match oidc.login_states.remove(&q.state) {
    Some((_, p)) => p,
    None => return (StatusCode::BAD_REQUEST, "Invalid/expired state").into_response(),
  };

  // Check if state = csrf_state per docs
  if pending.provider != provider {
    return (StatusCode::BAD_REQUEST, "Provider mismatch").into_response();
  }

  if pending.created_at.elapsed().as_secs() > LOGIN_STATE_TTL_SECS {
    return (StatusCode::BAD_REQUEST, "Login expired").into_response();
  }

  // 2) Get provider client
  let prov = match oidc.oidc_providers.get(&provider) {
    Some(p) => p,
    None => return (StatusCode::NOT_FOUND, "Unknown OIDC provider").into_response(),
  };

  let client = &prov.client;

  // 3) Exchange code -> tokens (client_secret verified here by IdP)
  let token_request = match client.exchange_code(AuthorizationCode::new(q.code)) {
    Ok(req) => req,
    Err(e) => {
      warn!("token endpoint not set / exchange_code failed: {e}");
      return (StatusCode::BAD_REQUEST, "OIDC token endpoint not available").into_response();
    }
  };

  let token_response = match token_request
    .request_async(&oidc.http_client)
    .await
  {
    Ok(t) => t,
    Err(e) => {
      warn!("token exchange failed: {e}");
      return (StatusCode::UNAUTHORIZED, "Token exchange failed").into_response();
    }
  };

  // 4) Verify ID token signature + nonce
  let id_token = match token_response.id_token() {
    Some(t) => t,
    None => return (StatusCode::UNAUTHORIZED, "Missing id_token").into_response(),
  };

  let claims = match id_token.claims(&client.id_token_verifier(), &pending.nonce) {
    Ok(c) => c,
    Err(e) => {
      warn!("id_token verification failed: {e}");
      return (StatusCode::UNAUTHORIZED, "Invalid id_token").into_response();
    }
  };

  let sub = claims.subject().as_str().to_owned();
  let Some(email) = claims.email().map(|e| e.as_str().to_owned()) else {
    return (StatusCode::BAD_REQUEST, "Missing email").into_response();
  };

  debug!("OIDC login ok provider={} sub={} email={:?}", provider, sub, email);

  // 5) Find existing identity by (provider, sub)
  match db::oidc::get_user_by_oidc_subject(state.pool.get().await.unwrap(), provider.clone(), sub.clone()).await {
    Ok(Some(user)) => {
      let claims = Claims::new(user.id);
      return issue_login_response(state.pool, claims).await.into_response();
    }

    // Continue to create a user
    Ok(None) => {}

    Err(e) => {
      error!("DB error selecting oidc identity: {e}");
      return StatusCode::INTERNAL_SERVER_ERROR.into_response();
    }
  }

  // 5b) If enabled, try to link an existing LOCAL user by email
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
          return (StatusCode::UNAUTHORIZED, "Email not verified").into_response();
        }

        // Link identity: (provider, sub) -> existing user id
        match db::oidc::insert_oidc_identity_link(
          state.pool.get().await.unwrap(),
          existing.id,
          provider.clone(),
          sub.clone(),
        ).await {
          Ok(()) => {
            return issue_login_response(state.pool, Claims::new(existing.id)).await.into_response();
          }
          Err(e) => {
            error!("DB error inserting oidc identity link: {e}");
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
          }
        }
      }
      // no existing user by email -> continue to signup gate
      Ok(None) => {}
      Err(e) => {
        error!("DB error selecting user by email: {e}");
        return StatusCode::INTERNAL_SERVER_ERROR.into_response();
      }
    }
  }

  // 6) Not found → signup gate
  if !prov.config.allow_signup {
    return (StatusCode::UNAUTHORIZED, "Signups disabled").into_response();
  }

  // 7) Create new local user (OIDC-only) + link identity
  let Ok(user_id) = insert_oidc_user(state.pool.get().await.unwrap(), provider.clone(), sub.clone(), email).await else {
    debug!("Created a new OIDC-only user: {} - {}", provider, sub);
    return (StatusCode::INTERNAL_SERVER_ERROR, "Can't create new oidc-only user").into_response();
  };

  // 8) Issue normal JWT login response
  issue_login_response(state.pool, Claims::new(user_id)).await.into_response()
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
