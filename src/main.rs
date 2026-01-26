#![warn(
  clippy::doc_markdown,
  clippy::unused_self,
  unused_extern_crates,
  unused_qualifications
)]

#![allow(
  clippy::manual_range_contains,
  clippy::too_many_arguments
)]

#[macro_use]
extern crate diesel;

use axum_extra::routing::RouterExt;
use dashmap::DashMap;
use diesel_migrations::{embed_migrations, EmbeddedMigrations};
use openapi::ApiDoc;
use tracing::{error, info, warn};
use utoipa_swagger_ui::SwaggerUi;
use crate::auth::secret::Secret;
use crate::directories::Directories;
use axum::{response::{Html, IntoResponse}, routing::get, Router, http::Request, middleware::{Next, self}, extract::{MatchedPath}, body::Body};
use deadpool_diesel::{Pool, Runtime, Manager};
use diesel::{MysqlConnection};
use diesel_migrations::MigrationHarness;
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use std::{future::ready, net::SocketAddr, process, sync::Arc, time::Instant};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tower_http::trace::TraceLayer;
use openidconnect::reqwest;

// mod media;
// mod errors;
mod routes;
mod models;
mod db;
mod scan;
mod schema;
mod auth;
mod directories;
mod oidc;
mod openapi;

pub type ConnectionPool = Pool<Manager<MysqlConnection>>;
pub type DbConn = deadpool::managed::Object<Manager<MysqlConnection>>;

async fn create_db_pool() -> Result<ConnectionPool, Box<dyn std::error::Error>> {
  let Ok(database_url) = std::env::var("DATABASE_URL") else {
    return Err(format!("DATABASE_URL not set").into());
  };

  let manager = Manager::<MysqlConnection>::new(database_url, Runtime::Tokio1);

  let pool = Pool::builder(manager)
    .max_size(8)
    .build()?;

  Ok(pool)
}

#[derive(Clone)]
pub struct OidcProvider {
  pub key: String,
  pub display_name: Option<String>,
  pub client: oidc::ConfiguredCoreClient,
  pub config: OidcProviderConfig,
}

#[derive(Clone)]
pub struct OidcProviderConfig {
  pub allow_signup: bool,
  // pub map_by_email: bool,
}

#[derive(Clone)]
pub struct AppState {
  pub pool: ConnectionPool,
  pub oidc_providers: Arc<DashMap<String, OidcProvider>>,
  pub login_states: Arc<DashMap<String, oidc::PendingLogin>>,
  pub http_client: reqwest::Client,
}

#[tokio::main]
async fn main() {
  // Load environmental variables from .env if present (e.g. development environment)
  dotenv::dotenv().ok();

  tracing_subscriber::registry()
    .with(
      tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "info,tower_http=debug".into()),
    )
    .with(tracing_subscriber::fmt::layer())
    .init();

  let dir = Directories::new();
  if dir.is_none() { panic!("Directories check failed."); }

  let secret_check = check_secret_startup();
  if secret_check.is_err() {
    panic!("Secret couldn't be read and/or created: {}", secret_check.unwrap_err());
  }

  let recorder_handle = setup_metrics_recorder();

  let pool = match create_db_pool().await {
    Ok(pool) => pool,
    Err(e) => {
      error!("Couldn't connect to DB: {e}");
      error!("Stopping server!");
      process::exit(1)
    }
  };

  pub const MIGRATIONS: EmbeddedMigrations = embed_migrations!();
  let _ = pool.get().await.unwrap().interact(|c| c.run_pending_migrations(MIGRATIONS).map(|_| ())).await.expect("Can't run migrations.");


  let http_client = reqwest::ClientBuilder::new()
    // Following redirects opens the client up to SSRF vulnerabilities.
    .redirect(reqwest::redirect::Policy::none())
    .build()
    .expect("Client should build");

    // Build OIDC provider map (for now: single provider from ENV)
  let oidc_providers: Arc<DashMap<String, OidcProvider>> = Arc::new(DashMap::new());

  match oidc::build_oidc_client(&http_client).await {
    Ok(client) => {
      let display_name = std::env::var("OIDC_PROVIDER_KEY").ok();

      // let map_by_email = std::env::var("OIDC_MAP_BY_EMAIL")
      //   .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1"))
      //   .unwrap_or(false);
      let allow_signup = std::env::var("OIDC_ALLOW_SIGNUP")
        .map(|v| matches!(v.to_lowercase().as_str(), "true" | "1"))
        .unwrap_or(false);

      if let Ok(oidc_provider) = std::env::var("OIDC_PROVIDER_KEY") {
        oidc_providers.insert(oidc_provider.clone(), OidcProvider { key: oidc_provider.clone(), display_name, client, config: OidcProviderConfig { allow_signup } });

        info!("OIDC enabled for provider: {:?}", oidc_provider);
      }
    }
    Err(e) => {
      // Don't crash server — just disable SSO
      warn!("OIDC disabled (startup discovery failed): {e}");
    }
  }

  let state = AppState {
    pool:pool.clone(),
    oidc_providers,
    login_states: Arc::new(DashMap::new()),
    http_client,
  };

  let protected = Router::new()
    .typed_get(routes::media::media_structure)
    .typed_get(routes::media::get_media_by_uuid)
    .typed_post(routes::albums::create_album)
    .typed_get(routes::albums::album_add_media)
    .typed_get(routes::albums::get_album_list)
    .typed_put(routes::albums::update_album)
    .typed_delete(routes::albums::delete_album)
    .typed_put(routes::media::media_update_description)
    .typed_delete(routes::media::media_delete_description)
    .typed_get(routes::media::get_media_liked_list)
    .typed_post(routes::media::media_like)
    .typed_delete(routes::media::media_unlike)
    .typed_get(routes::albums::get_album_share_links)
    .typed_post(routes::albums::create_album_share_link)
    .typed_put(routes::albums::update_album_share_link)
    .typed_delete(routes::albums::delete_album_share_link)
    .typed_get(routes::scan_media)
    .route_layer(middleware::from_fn_with_state(state.clone(), auth::token::auth));

  let unprotected = Router::new()
    .route("/", get(handler))
    .typed_get(routes::health)
    .route("/metrics", get(move || ready(recorder_handle.render())))
    .typed_get(routes::oidc::get_server_config)
    .typed_get(routes::oidc::oidc_login)
    .typed_get(routes::oidc::oidc_callback)
    .typed_post(routes::create_user)
    .typed_post(routes::login)
    .typed_post(routes::refresh_token)
    .typed_get(routes::albums::get_album_share_link)
    .typed_get(routes::system_info_public);

  let mixed_auth = Router::new()
    .typed_get(routes::albums::get_album_structure)
    .route_layer(middleware::from_fn_with_state(state.clone(), auth::mixed_auth::mixed_auth));

  // build our application with a route
  let app = protected
    .merge(unprotected)
    .merge(mixed_auth)
    .merge(SwaggerUi::new("/swagger-ui")
      .url("/openapi.json", ApiDoc::generate_openapi())
      .url("/openapi-tagless.json", ApiDoc::generate_openapi_tagless())
    )
    .route_layer(middleware::from_fn(track_metrics))
    .layer(TraceLayer::new_for_http())
    .with_state(state);

  // run it
  let addr = SocketAddr::from(([0, 0, 0, 0], 8000));
  info!("listening on http://{}", addr);
  let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
  axum::serve(listener, app).await.unwrap();
}

async fn handler() -> Html<&'static str> {
  Html("<h1>Hello, World!</h1>")
}

fn setup_metrics_recorder() -> PrometheusHandle {
  const EXPONENTIAL_SECONDS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
  ];

  PrometheusBuilder::new()
    .set_buckets_for_metric(
      Matcher::Full("http_requests_duration_seconds".to_string()),
      EXPONENTIAL_SECONDS,
    )
    .unwrap()
    .install_recorder()
    .unwrap()
}

async fn track_metrics(req: Request<Body>, next: Next) -> impl IntoResponse {
    let start = Instant::now();
    let path = if let Some(matched_path) = req.extensions().get::<MatchedPath>() {
        matched_path.as_str().to_owned()
    } else {
        req.uri().path().to_owned()
    };
    let method = req.method().clone();

    let response = next.run(req).await;

    let latency = start.elapsed().as_secs_f64();
    let status = response.status().as_u16().to_string();

    let labels = [
        ("method", method.to_string()),
        ("path", path),
        ("status", status),
    ];

    metrics::counter!("http_requests_total", &labels).increment(1);
    metrics::histogram!("http_requests_duration_seconds", &labels).record(latency);

    response
}

/// Checks whether the secret.key file is present and tries to create it if it isn't.\
/// This is meant to be run before starting Rocket.
pub fn check_secret_startup() -> Result<(), std::io::Error> {
  let read = Secret::read();
  if read.is_err() {
    Secret::new().write()?;

    // It is also possible to have write-only access, so we must check reading too.
    Secret::read()?;

    warn!("Created missing secret.key file.");
  }

  info!("The secret.key file was successfully read.");
  Ok(())
}
