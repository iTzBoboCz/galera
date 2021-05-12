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
use diesel::MysqlConnection;

// mod media;
// mod errors;
mod handlers;
mod models;
mod schema;
mod scan;

pub type Pool = r2d2::Pool<ConnectionManager<MysqlConnection>>;
pub type Manager = ConnectionManager::<MysqlConnection>;

embed_migrations!();

#[actix_web::main]
async fn main() -> std::io::Result<()> {
  env_logger::init();

  dotenv::dotenv().ok();
  std::env::set_var("RUST_LOG", "actix_web=debug");
  let database_url = std::env::var("DATABASE_URL").expect("DATABASE_URL must be set");

  // create db connection pool
  let manager = Manager::new(database_url);
  let pool: Pool = r2d2::Pool::builder().build(manager).unwrap();

  let migration = embedded_migrations::run(&pool.clone().get().expect("Failed to migrate."));

  match migration {
    Ok(_) => info!("Migration succesful."),
    Err(_) => warn!("Failed to migrate."),
  }

  let xdg_data = "gallery";
  let username = "ondrejpesek";
  scan::scan_root(xdg_data, username);

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
