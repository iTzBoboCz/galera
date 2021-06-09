#[macro_use]
extern crate diesel;

#[macro_use]
extern crate rocket;

#[macro_use]
extern crate log;

#[macro_use]
extern crate diesel_migrations;

use std::thread;
use rocket_sync_db_pools::database;
use diesel_migrations::embed_migrations;
use rocket::{Rocket, Build};
use rocket::fairing::AdHoc;

// mod media;
// mod errors;
// mod db;
// mod handlers;
mod models;
//mod scan;
mod schema;

#[database("galera")]
struct DbConn(diesel::MysqlConnection);

#[get("/")]
async fn index() -> &'static str {
  "Hello, world!"
}

#[get("/scan_media")]
async fn scan_media(conn: DbConn) -> &'static str {
  let xdg_data = "gallery";
  let user_id: i32 = 1;

  // this thread will run until scanning is complete
  thread::spawn(|| {

    // scan::scan_root(conn, xdg_data, user_id);
  });

  "immediate response"
}

#[launch]
fn rocket() -> _ {
  env_logger::init();

  dotenv::dotenv().ok();
  // std::env::set_var("RUST_LOG", "actix_web=debug");

  rocket::build()
    .attach(DbConn::fairing())
    .attach(AdHoc::on_ignite("Database migration", run_migrations))
    .mount("/", routes![index])
    .mount("/", routes![scan_media])
}

/// Runs migrations
pub async fn run_migrations(rocket: Rocket<Build>) -> Rocket<Build> {

  embed_migrations!();

  let conn = DbConn::get_one(&rocket).await.expect("database connection");
  conn.run(|c| embedded_migrations::run(c)).await.expect("can run migrations");

  rocket
}
