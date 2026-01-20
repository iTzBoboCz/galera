use std::time::Instant;
use crate::auth::token::Claims;
use crate::db::oidc::insert_oidc_user;
use crate::routes::issue_login_response;
use axum::extract::{Query, State};
use axum::response::{IntoResponse, Redirect};
use axum::{Json, http::StatusCode};
use axum_extra::routing::TypedPath;
use openidconnect::core::CoreAuthenticationFlow;
use openidconnect::{AuthorizationCode, CsrfToken, Nonce, Scope, TokenResponse};
use serde::{Deserialize, Serialize};
use tracing::{debug, error, warn};
use crate::{AppState, db};

#[derive(TypedPath, Deserialize)]
#[typed_path("/auth/oidc/:provider/login")]
pub struct OidcLogin {
  pub provider: String,
}

pub async fn oidc_login(
  OidcLogin { provider }: OidcLogin,
  State(state): State<AppState>,
) -> impl IntoResponse {
  let prov = match state.oidc_providers.get(&provider) {
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
  state.login_states.insert(
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
#[typed_path("/auth/oidc/:provider/callback")]
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

pub async fn oidc_callback(
  OidcCallback { provider }: OidcCallback,
  Query(q): Query<OidcCallbackQuery>,
  State(state): State<AppState>
) -> impl IntoResponse {
  // Validate and consume CSRF "state"
  let pending = match state.login_states.remove(&q.state) {
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
  let prov = match state.oidc_providers.get(&provider) {
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
    .request_async(&state.http_client)
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

#[derive(Serialize, Deserialize)]
pub struct ServerConfigResponse {
  auth: AuthConfig,
}

#[derive(Serialize, Deserialize)]
pub struct AuthConfig {
    pub oidc: Vec<OidcProviderPublic>,
}

#[derive(Serialize, Deserialize)]
pub struct OidcProviderPublic {
    pub key: String,
    pub display_name: String,
    pub login_url: String,
}

#[derive(TypedPath)]
#[typed_path("/public/config")]
pub struct ServerConfig;

/// Returns server configuration
pub async fn get_server_config(
  _: ServerConfig,
  State(AppState { oidc_providers,..}): State<AppState>,
) -> Json<ServerConfigResponse> {

  let oidc = oidc_providers
    .iter()
    .map(|entry| {
      let provider = entry.value();

      let key = provider.key.clone();

      // use OIDC_PROVIDER_KEY when OIDC_DISPLAY_NAME isn't available
      let display_name = provider
        .display_name
        .clone()
        .unwrap_or_else(|| key.clone());

      OidcProviderPublic {
        display_name,
        login_url: format!("/auth/oidc/{}/login", key),
        key,
      }
    })
    .collect::<Vec<_>>();

  Json(ServerConfigResponse {
    auth: AuthConfig { oidc },
  })

}
