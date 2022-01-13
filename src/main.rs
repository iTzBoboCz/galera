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
extern crate rocket;

#[macro_use]
extern crate rocket_okapi;

#[macro_use]
extern crate log;

#[macro_use]
extern crate diesel_migrations;

use rocket_okapi::swagger_ui::{ make_swagger_ui, SwaggerUIConfig };
use rocket_sync_db_pools::database;
use diesel_migrations::embed_migrations;
use rocket::{Rocket, Build};
use rocket::fairing::AdHoc;
use crate::auth::secret::Secret;
use crate::directories::Directories;

// mod media;
// mod errors;
mod db;
mod routes;
mod models;
mod scan;
mod schema;
mod auth;
mod directories;

/// Connection to the database.
#[database("galera")]
pub struct DbConn(diesel::MysqlConnection);

#[launch]
fn rocket() -> _ {
  env_logger::init();

  dotenv::dotenv().ok();

  let dir = Directories::new();
  if dir.is_none() { panic!("Directories check failed."); }

  let secret_check = check_secret_startup();
  if secret_check.is_err() {
    panic!("Secret couldn't be read and/or created: {}", secret_check.unwrap_err());
  }

  rocket::build()
    .attach(DbConn::fairing())
    .attach(AdHoc::on_ignite("Database migration", run_migrations))
    // routes_with_openapi![...] will host the openapi document at openapi.json
    .mount(
      "/",
      openapi_get_routes![
        routes::index,
        routes::media_structure,
        routes::scan_media,
        routes::get_media_by_uuid,
        routes::create_user,
        routes::get_album_list,
        routes::create_album,
        routes::update_album,
        routes::delete_album,
        routes::album_add_media,
        routes::login,
        routes::refresh_token,
        routes::get_media_liked_list,
        routes::get_album_structure,
        routes::media_like,
        routes::media_unlike,
        routes::system_info_public,
        routes::media_update_description,
        routes::media_delete_description
      ],
    )
    .mount(
      "/swagger-ui/",
      make_swagger_ui(&SwaggerUIConfig {
        url: "../openapi.json".to_owned(),
        ..Default::default()
      }),
    )
}

/// Runs migrations
pub async fn run_migrations(rocket: Rocket<Build>) -> Rocket<Build> {

  embed_migrations!();

  let conn = DbConn::get_one(&rocket).await.expect("database connection");
  conn.run(|c| embedded_migrations::run(c)).await.expect("can run migrations");

  rocket
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
