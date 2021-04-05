#[macro_use] extern crate diesel;
extern crate r2d2;

#[macro_use]
extern crate log;

#[allow(unused_imports)]
use actix_web::{ web, App, HttpServer, Responder, middleware };
use diesel::SqliteConnection;
use diesel::r2d2::ConnectionManager;

// mod media;
// mod errors;
mod handlers;
mod models;
mod schema;

pub type Pool = r2d2::Pool<ConnectionManager<SqliteConnection>>;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  env_logger::init();

  dotenv::dotenv().ok();
  std::env::set_var("RUST_LOG", "actix_web=debug");
  let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

  // create db connection pool
  let manager = ConnectionManager::<SqliteConnection>::new(database_url);
  let pool: Pool = r2d2::Pool::builder()
  .build(manager)
  .unwrap();

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
