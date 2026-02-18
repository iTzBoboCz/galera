#![allow(unused_qualifications)]

pub mod albums;
pub mod folders;
pub mod media;
pub mod tokens;
pub mod users;
pub mod oidc;

use axum::http::StatusCode;
use deadpool_diesel::{InteractError, PoolError};
use diesel::{MysqlConnection, RunQueryDsl};
use thiserror::Error;
use tracing::warn;

use crate::{ConnectionPool, DbConn};

pub struct CheckedDbConn<'a> {
  pool: &'a ConnectionPool,
  conn: DbConn,
}

pub async fn get_db(pool: &ConnectionPool) -> Result<CheckedDbConn<'_>, DbError> {
  let conn = pool.get().await.map_err(DbError::Pool)?;
  Ok(CheckedDbConn { pool, conn })
}

pub async fn interact_diesel<T, F>(conn: DbConn, f: F) -> Result<T, DbError>
where
  T: Send + 'static,
  F: FnOnce(&mut MysqlConnection) -> Result<T, diesel::result::Error> + Send + 'static,
{
  let out = conn
    .interact(f)
    .await
    .map_err(DbError::Interact)??;

  Ok(out)
}

impl<'a> CheckedDbConn<'a> {
  pub async fn run<T, Fut, F>(self, op: F) -> Result<T, DbError>
  where
    F: Fn(DbConn) -> Fut,
    Fut: Future<Output = Result<T, DbError>>,
  {
    let pool = self.pool;
    let conn1 = self.conn;

    match op(conn1).await {
      Ok(v) => Ok(v),
      Err(e) if e.is_stale() => {
        warn!("Stale DB connection detected, retrying once: {e}");

        let conn2 = pool.get().await.map_err(DbError::Pool)?;
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

pub async fn healthcheck(conn: DbConn) -> Result<(), DbError> {
  interact_diesel(conn, |c| {
    diesel::sql_query("SELECT 1").execute(c)?;
    Ok(())
  })
  .await
}

/// Used for getting last inserted id.
#[derive(diesel::deserialize::QueryableByName)]
pub struct LastInsertId {
  #[diesel(sql_type = diesel::sql_types::Integer)]
  pub id: i32,
}

#[derive(Debug, Error)]
pub enum DbError {
  // deadpool couldn't hand out a connection
  #[error("pool error: {0}")]
  Pool(#[from] PoolError),

  // deadpool couldn't run your blocking closure (panic/cancel/etc.)
  #[error("interact error: {0}")]
  Interact(#[from] InteractError),

  // real Diesel error (SQL constraint, not found, db went away, etc.)
  #[error("diesel error: {0}")]
  Diesel(#[from] diesel::result::Error),
}

impl DbError {
  pub fn is_stale(&self) -> bool {
    match self {
      DbError::Diesel(e) => is_stale_mysql_conn(e),
      DbError::Interact(_) => false,
      DbError::Pool(_) => false,
    }
  }
}

impl From<DbError> for StatusCode {
  fn from(e: DbError) -> Self {
    match e {
      DbError::Pool(_) | DbError::Interact(_) => StatusCode::SERVICE_UNAVAILABLE,

      DbError::Diesel(d) => match d {
        diesel::result::Error::NotFound => StatusCode::NOT_FOUND,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
      },
    }
  }
}
