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

#[macro_use]
extern crate diesel_migrations;

use axum_extra::routing::RouterExt;
use diesel_migrations::embed_migrations;
// use crate::auth::secret::Secret;
use crate::directories::Directories;
use axum::{response::{Html, IntoResponse}, routing::get, Router, http::Request, middleware::{Next, self}, extract::{MatchedPath, State}};
use deadpool_diesel::{Pool, Runtime, Manager};
use diesel::{MysqlConnection};
use metrics_exporter_prometheus::{Matcher, PrometheusBuilder, PrometheusHandle};
use std::{net::SocketAddr, time::Instant, future::ready};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use tower_http::trace::TraceLayer;

// mod media;
// mod errors;
mod routes;
mod models;
mod db;
mod scan;
mod schema;
mod auth;
mod directories;

pub type ConnectionPool = Pool<Manager<MysqlConnection>>;
pub type DbConn = deadpool::managed::Object<Manager<MysqlConnection>>;

async fn create_db_pool() -> ConnectionPool {
  let manager = Manager::<MysqlConnection>::new("mysql://root:root@localhost/galera", Runtime::Tokio1);

  Pool::builder(manager)
    .max_size(8)
    .build()
    .unwrap()
}

#[tokio::main]
async fn main() {
  tracing_subscriber::registry()
    .with(tracing_subscriber::EnvFilter::new(
        std::env::var("RUST_LOG")
          .unwrap_or_else(|_| "example_tracing_aka_logging=debug,tower_http=debug".into()),
    ))
    .with(tracing_subscriber::fmt::layer())
    .init();

  let dir = Directories::new();
  if dir.is_none() { panic!("Directories check failed."); }

  let recorder_handle = setup_metrics_recorder();

  let pool = create_db_pool().await;

  embed_migrations!();
  pool.get().await.unwrap().interact(|c| embedded_migrations::run(c)).await.expect("Can't connect to the database.").expect("Can't run migrations.");


  let protected = Router::new()
    .typed_get(routes::get_media_by_uuid)
    .typed_post(routes::create_album)
    .typed_get(routes::album_add_media)
    .typed_get(routes::get_album_list)
    .typed_put(routes::update_album)
    .typed_delete(routes::delete_album)
    .typed_put(routes::media_update_description)
    .typed_delete(routes::media_delete_description)
    .route_layer(middleware::from_fn_with_state(pool.clone(), auth::token::auth));

  let unprotected = Router::new()
    .route("/", get(handler))
    .route("/metrics", get(move || ready(recorder_handle.render())))
    .typed_post(routes::create_user)
    .typed_post(routes::login)
    .typed_post(routes::refresh_token)
    .typed_get(routes::system_info_public);

  let mixed_auth = Router::new()
    .typed_get(routes::get_album_structure)
    .route_layer(middleware::from_fn_with_state(pool.clone(), auth::token::mixed_auth));

  // build our application with a route
  let app = protected
    .merge(unprotected)
    .merge(mixed_auth)
    .route_layer(middleware::from_fn(track_metrics))
    .layer(TraceLayer::new_for_http())
    .with_state(pool);

  // run it
  let addr = SocketAddr::from(([127, 0, 0, 1], 8000));
  println!("listening on {}", addr);
  axum::Server::bind(&addr)
    .serve(app.into_make_service())
    .await
    .unwrap();
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

async fn track_metrics<B>(req: Request<B>, next: Next<B>) -> impl IntoResponse {
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

    metrics::increment_counter!("http_requests_total", &labels);
    metrics::histogram!("http_requests_duration_seconds", latency, &labels);

    response
}

// /// Connection to the database.
// #[database("galera")]
// pub struct DbConn(diesel::MysqlConnection);

// #[launch]
// fn rocket() -> _ {
//   env_logger::init();

//   dotenv::dotenv().ok();

//   let dir = Directories::new();
//   if dir.is_none() { panic!("Directories check failed."); }

//   let secret_check = check_secret_startup();
//   if secret_check.is_err() {
//     panic!("Secret couldn't be read and/or created: {}", secret_check.unwrap_err());
//   }

//   rocket::build()
//     .attach(DbConn::fairing())
//     .attach(AdHoc::on_ignite("Database migration", run_migrations))
//     // routes_with_openapi![...] will host the openapi document at openapi.json
//     .mount(
//       "/",
//       openapi_get_routes![
//         routes::index,
//         routes::media_structure,
//         routes::scan_media,
//         routes::get_media_by_uuid,
//         routes::create_user,
//         routes::get_album_list,
//         routes::create_album,
//         routes::update_album,
//         routes::delete_album,
//         routes::album_add_media,
//         routes::login,
//         routes::refresh_token,
//         routes::get_media_liked_list,
//         routes::get_album_structure,
//         routes::media_like,
//         routes::media_unlike,
//         routes::system_info_public,
//         routes::media_update_description,
//         routes::media_delete_description,
//         routes::create_album_share_link,
//         routes::get_album_share_links,
//         routes::get_album_share_link,
//         routes::update_album_share_link,
//         routes::delete_album_share_link
//       ],
//     )
//     .mount(
//       "/swagger-ui/",
//       make_swagger_ui(&SwaggerUIConfig {
//         url: "../openapi.json".to_owned(),
//         ..Default::default()
//       }),
//     )
// }

// /// Checks whether the secret.key file is present and tries to create it if it isn't.\
// /// This is meant to be run before starting Rocket.
// pub fn check_secret_startup() -> Result<(), std::io::Error> {
//   let read = Secret::read();
//   if read.is_err() {
//     Secret::new().write()?;

//     // It is also possible to have write-only access, so we must check reading too.
//     Secret::read()?;

//     warn!("Created missing secret.key file.");
//   }

//   info!("The secret.key file was successfully read.");
//   Ok(())
// }
