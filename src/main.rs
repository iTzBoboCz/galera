#[macro_use]
extern crate diesel;
extern crate r2d2;

#[macro_use]
extern crate log;

#[macro_use]
extern crate diesel_migrations;

#[allow(unused_imports)]
use actix_web::{ web, App, HttpServer, Responder, middleware };
use diesel::r2d2::ConnectionManager;
use diesel::SqliteConnection;
use infer::{is_audio, is_video};
use walkdir::WalkDir;

// mod media;
// mod errors;
mod handlers;
mod models;
mod schema;
mod scan;

pub type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

embed_migrations!();

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  env_logger::init();

  dotenv::dotenv().ok();
  std::env::set_var("RUST_LOG", "actix_web=debug");
  let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

  // create db connection pool
  let manager = ConnectionManager::<SqliteConnection>::new(database_url);
  let pool: Pool = r2d2::Pool::builder().build(manager).unwrap();

  let migration = embedded_migrations::run(&pool.clone().get().expect("Failed to migrate."));

  match migration {
    Ok(_) => info!("Migration succesful."),
    Err(_) => warn!("Failed to migrate."),
  }

  use std::{fs, io};
  use std::path::{PathBuf, Path};

  let xdg_data = "gallery";
  let username = "ondrejpesek";

  let root = format!("{}/{}/", xdg_data, username);

  for entry in WalkDir::new(&root).into_iter().filter_map(|e| e.ok()) {
    // https://stackoverflow.com/questions/30309100/how-to-check-if-a-given-path-is-a-file-or-directory#comment105412329_30309566
    let mut found_folders: Vec<&str>;
    if entry.path().is_dir() {
      let string = entry.path().display().to_string();
      let string_stripped = string.strip_prefix(&root).unwrap();

      if string_stripped == "" { continue };

      let string_split = string_stripped.split("/").map(String::from).collect::<Vec<String>>();

      // skip if folder doesn't contain any pictures or videos
      if string_split.len() == 1 {
        if !scan::folder_has_media(PathBuf::from(string)) { continue };
      }

      warn!("{:?}", string_split);
      for s in string_split {
        warn!("{:?}", s);
        // found_folders.push(s);
      }
    }
    // warn!("{:?}", found_folders);
  }

  HttpServer::new(move || {
    App::new()
      .wrap(middleware::Logger::default())
      .data(pool.clone())
      .route("/user/username/{name}", web::get().to(handlers::test))
  })
  .bind("localhost:3030")?
  .run()
  .await
}
