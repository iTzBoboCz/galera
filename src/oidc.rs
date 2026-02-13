use openidconnect::{
    ClientId, ClientSecret, EndpointMaybeSet, EndpointNotSet, EndpointSet, IssuerUrl, RedirectUrl, core::{CoreClient, CoreProviderMetadata}, reqwest
};
use tracing::debug;

use std::time::Instant;
use openidconnect::Nonce;

use crate::config::{get_backend_url, get_frontend_url};

/// Stores temporary data between /login and /callback
#[derive(Clone)]
pub struct PendingLogin {
    pub provider: String,
    pub nonce: Nonce,
    pub created_at: Instant,
}

pub type ConfiguredCoreClient = CoreClient<
    EndpointSet,      // auth url set
    EndpointNotSet,   // device auth url not set
    EndpointNotSet,   // introspection url not set
    EndpointNotSet,   // revocation url not set
    EndpointMaybeSet, // token url maybe set (depends on provider metadata)
    EndpointMaybeSet, // userinfo url maybe set (depends on provider metadata)
>;

pub async fn build_oidc_client(http_client: &reqwest::Client) -> Result<ConfiguredCoreClient, Box<dyn std::error::Error>> {
    let issuer = std::env::var("OIDC_ISSUER")?;
    let client_id = std::env::var("OIDC_CLIENT_ID")?;
    let client_secret = std::env::var("OIDC_CLIENT_SECRET")?;
    let provider_key = std::env::var("OIDC_PROVIDER_KEY")?;
    if [&issuer, &client_id, &client_secret, &provider_key].iter().any(|v| v.trim().is_empty()) {
      debug!("One or more OIDC environmental variables empty");
      return Err(format!("One or more OIDC environmental variables empty").into());
    }

    let backend_url = get_backend_url().ok_or("BACKEND_URL not set or invalid")?;
    let _frontend_url = get_frontend_url().ok_or("FRONTEND_URL not set or invalid")?;

    let redirect = std::env::var("OIDC_REDIRECT_URL")
        .unwrap_or_else(|_| format!("{}auth/oidc/{}/callback", backend_url.as_str(), provider_key).to_string());

    // IMPORTANT: issuer should be like: https://auth.example.com/realms/YourRealm
    let provider_metadata = CoreProviderMetadata::discover_async(
        IssuerUrl::new(issuer)?,
        http_client,
    )
    .await?;

    let client = CoreClient::from_provider_metadata(
        provider_metadata,
        ClientId::new(client_id),
        Some(ClientSecret::new(client_secret)),
    )
    .set_redirect_uri(RedirectUrl::new(redirect)?);

    Ok(client)
}

// pub async fn verify_secret() {

// }
