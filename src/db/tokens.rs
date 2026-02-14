use crate::db::LastInsertId;
use crate::models::{NewAuthAccessToken, NewAuthRefreshToken};
use crate::{DbConn};
use crate::schema::{auth_access_token, auth_refresh_token};
use chrono::NaiveDateTime;
use diesel::{Connection, RunQueryDsl};
use diesel::QueryDsl;
use diesel::OptionalExtension;
use diesel::ExpressionMethods;
use tracing::error;

#[allow(dead_code)]
/// Inserts a new refresh token.
/// # Example
/// This will insert a new refresh token for a user with ID 1.
/// ```
/// insert_access_token(&conn, 1, "<my_access_token>".to_string());
/// ```
pub async fn insert_refresh_token(conn: DbConn, user_id: i32, refresh_token: String) -> Result<i32, diesel::result::Error> {
  conn.interact(move |c| {
    diesel::insert_into(auth_refresh_token::table)
      .values(NewAuthRefreshToken::new(user_id, refresh_token))
      .execute(c)?;

    let LastInsertId { id: refresh_token_id } =
      diesel::sql_query("SELECT LAST_INSERT_ID() AS id")
        .get_result::<LastInsertId>(c)?;

    Ok(refresh_token_id)
  }).await
  .map_err(|_| diesel::result::Error::RollbackTransaction)?
}

/// Selects (refresh_token_id, user_id) from a given refresh token value.
pub async fn select_refresh_token_session(
  conn: DbConn,
  refresh_token: String,
) -> Result<Option<(i32, i32)>, diesel::result::Error> {
  let result = conn
    .interact(move |c| {
      auth_refresh_token::table
        .select((auth_refresh_token::id, auth_refresh_token::user_id))
        .filter(auth_refresh_token::refresh_token.eq(refresh_token))
        .first::<(i32, i32)>(c)
        .optional()
    })
    .await
    .map_err(|e| {
      error!("DB interact failed in select_refresh_token_session: {e}");
      diesel::result::Error::DatabaseError(
        diesel::result::DatabaseErrorKind::Unknown,
        Box::new(format!("interact failed: {e}")),
      )
    })??;

  Ok(result)
}

/// Selects expiration time from a given refresh token.
pub async fn select_refresh_token_expiration(conn: &mut DbConn, refresh_token: String) -> Option<NaiveDateTime> {
  conn.interact(move |c| {
    auth_refresh_token::table
      .select(auth_refresh_token::expiration_time)
      .filter(auth_refresh_token::refresh_token.eq(refresh_token))
      .first(c)
      .optional()
      .unwrap()
  }).await.unwrap()
}

/// Inserts a new token.
/// # Example
/// This will insert a new access token with refresh token ID 20.
/// ```
/// insert_access_token(&conn, 20, "<my_access_token>".to_string());
/// ```
pub async fn insert_access_token(conn: DbConn, refresh_token_id: i32, access_token: String) -> Result<i32, diesel::result::Error> {
  conn.interact(move |c| {
    diesel::insert_into(auth_access_token::table)
      .values(NewAuthAccessToken::new(refresh_token_id, access_token))
      .execute(c)?;

    let LastInsertId { id: access_token_id } =
      diesel::sql_query("SELECT LAST_INSERT_ID() AS id")
        .get_result::<LastInsertId>(c)?;

    Ok(access_token_id)
  }).await
  .map_err(|_| diesel::result::Error::RollbackTransaction)?
}

/// Deletes obsolete access tokens for a given refresh token.
pub async fn delete_obsolete_access_tokens(conn: DbConn, refresh_token_id: i32) -> Result<usize, diesel::result::Error> {
  conn
    .interact(move |c| {
      diesel::delete(
        auth_access_token::table
          .filter(auth_access_token::refresh_token_id.eq(refresh_token_id)),
      )
      .execute(c)
    })
    .await
    .map_err(|_| diesel::result::Error::RollbackTransaction)?
}

pub async fn insert_session_tokens(conn: DbConn, user_id: i32, refresh_token: String, access_token: String) -> Result<(i32, i32), diesel::result::Error> {
  conn.interact(move |c| {
    c.transaction::<(i32, i32), diesel::result::Error, _>(|c| {
      diesel::insert_into(auth_refresh_token::table)
        .values(NewAuthRefreshToken::new(user_id, refresh_token))
        .execute(c)?;

      let LastInsertId { id: refresh_token_id } =
        diesel::sql_query("SELECT LAST_INSERT_ID() AS id")
          .get_result::<LastInsertId>(c)?;


      diesel::insert_into(auth_access_token::table)
        .values(NewAuthAccessToken::new(refresh_token_id, access_token))
        .execute(c)?;

      let LastInsertId { id: access_token_id } =
        diesel::sql_query("SELECT LAST_INSERT_ID() AS id")
          .get_result::<LastInsertId>(c)?;

      Ok((refresh_token_id, access_token_id))
    })
  }).await
  .map_err(|_| diesel::result::Error::RollbackTransaction)?
}


pub async fn delete_session_by_refresh_token(
  conn: DbConn,
  refresh_token: String,
) -> Result<bool, diesel::result::Error> {
  conn
    .interact(move |c| {
      c.transaction::<bool, diesel::result::Error, _>(|c| {
        let refresh_token_id = auth_refresh_token::table
          .select(auth_refresh_token::id)
          .filter(auth_refresh_token::refresh_token.eq(&refresh_token))
          .first::<i32>(c)
          .optional()?;

        let Some(refresh_token_id) = refresh_token_id else {
          return Ok(false);
        };

        // Delete all access tokens under this refresh token
        diesel::delete(
          auth_access_token::table.filter(auth_access_token::refresh_token_id.eq(refresh_token_id)),
        )
        .execute(c)?;

        // Delete the refresh token itself
        diesel::delete(
          auth_refresh_token::table.filter(auth_refresh_token::id.eq(refresh_token_id)),
        )
        .execute(c)?;

        Ok(true)
      })
    })
    .await
    .map_err(|_| diesel::result::Error::RollbackTransaction)?
}
