#![allow(unused_qualifications)]

pub mod albums;
pub mod folders;
pub mod media;
pub mod tokens;
pub mod users;
pub mod oidc;

use axum::http::StatusCode;
use diesel::{RunQueryDsl, dsl::sql, sql_types::Integer};
use tracing::{warn, error};

use crate::{ConnectionPool, DbConn};

pub struct CheckedDbConn<'a> {
  pool: &'a ConnectionPool,
  conn: DbConn,
}

pub async fn get_db(pool: &ConnectionPool) -> Result<CheckedDbConn<'_>, StatusCode> {
  let conn = pool.get().await.map_err(|e| {
    error!("DB pool.get failed: {e}");
    StatusCode::SERVICE_UNAVAILABLE
  })?;

  Ok(CheckedDbConn { pool, conn })
}

impl<'a> CheckedDbConn<'a> {
  pub async fn run<T, Fut, F>(self, op: F) -> Result<T, diesel::result::Error>
  where
    F: Fn(DbConn) -> Fut,
    Fut: std::future::Future<Output = Result<T, diesel::result::Error>>,
  {
    let pool = self.pool;
    let conn1 = self.conn;

    match op(conn1).await {
      Ok(v) => Ok(v),
      Err(e) if is_stale_mysql_conn(&e) => {
        warn!("Stale DB connection detected, retrying once: {e}");

        // conn1 already dropped here (out of scope), nothing to drop manually

        let conn2 = pool
          .get()
          .await
          .map_err(|_| diesel::result::Error::NotFound)?;
        op(conn2).await
      }
      Err(e) => Err(e),
    }
  }
}

fn is_stale_mysql_conn(e: &diesel::result::Error) -> bool {
  let s = e.to_string().to_ascii_lowercase();
  s.contains("server has gone away")
    || s.contains("lost connection")
    || s.contains("error reading communication packets")
    || s.contains("broken pipe")
    || s.contains("connection reset")
}

pub async fn healthcheck(conn: DbConn) -> Result<(), diesel::result::Error> {
  conn
    .interact(|c| {
      let _one: i32 = sql::<Integer>("SELECT 1").get_result(c)?;
      Ok::<(), diesel::result::Error>(())
    })
    .await
    .map_err(|_| diesel::result::Error::RollbackTransaction)?
}

/// Used for getting last inserted id.
#[derive(diesel::deserialize::QueryableByName)]
pub struct LastInsertId {
  #[diesel(sql_type = diesel::sql_types::Integer)]
  pub id: i32,
}
