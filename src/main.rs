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

// mod media;
// mod errors;
mod db;
mod routes;
mod models;
mod scan;
mod schema;

#[database("galera")]
pub struct DbConn(diesel::MysqlConnection);

#[launch]
fn rocket() -> _ {
  env_logger::init();

  dotenv::dotenv().ok();
  // std::env::set_var("RUST_LOG", "actix_web=debug");

  rocket::build()
    .attach(DbConn::fairing())
    .attach(AdHoc::on_ignite("Database migration", run_migrations))
    // routes_with_openapi![...] will host the openapi document at openapi.json
    .mount("/", routes_with_openapi![routes::index, routes::scan_media, routes::get_media_by_uuid, routes::test])
    .mount(
      "/swagger-ui/",
      make_swagger_ui(&SwaggerUIConfig {
        url: "../openapi.json".to_owned(),
        ..Default::default()
      })
    )
}

/// Runs migrations
pub async fn run_migrations(rocket: Rocket<Build>) -> Rocket<Build> {

  embed_migrations!();

  let conn = DbConn::get_one(&rocket).await.expect("database connection");
  conn.run(|c| embedded_migrations::run(c)).await.expect("can run migrations");

  rocket
}
