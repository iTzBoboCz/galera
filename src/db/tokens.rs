use crate::models::{NewAuthAccessToken, NewAuthRefreshToken};
use crate::{DbConn};
use crate::schema::{auth_access_token, auth_refresh_token};
use chrono::NaiveDateTime;
use diesel::RunQueryDsl;
use diesel::QueryDsl;
use diesel::OptionalExtension;
use diesel::ExpressionMethods;

/// Inserts a new refresh token.
/// # Example
/// This will insert a new refresh token for a user with ID 1.
/// ```
/// insert_access_token(&conn, 1, "<my_access_token>".to_string());
/// ```
pub async fn insert_refresh_token(conn: DbConn, user_id: i32, refresh_token: String) -> Option<()> {
  let r: Result<usize, diesel::result::Error> = conn.interact(move |c| {
    diesel::insert_into(auth_refresh_token::table)
      .values(NewAuthRefreshToken::new(user_id, refresh_token))
      .execute(c)
  }).await.unwrap();

  if r.is_err() {
    return None;
  }

  Some(())
}

/// Selects refresh token ID from a given refresh token.
pub async fn select_refresh_token_id(conn: DbConn, refresh_token: String) -> Option<i32> {
  conn.interact(move |c| {
    auth_refresh_token::table
      .select(auth_refresh_token::id)
      .filter(auth_refresh_token::refresh_token.eq(refresh_token))
      .first(c)
      .optional()
      .unwrap()
  }).await.unwrap()
}

/// Selects expiration time from a given refresh token.
pub async fn select_refresh_token_expiration(conn: DbConn, refresh_token: String) -> Option<NaiveDateTime> {
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
pub async fn insert_access_token(conn: DbConn, refresh_token_id: i32, access_token: String) -> Option<()> {
  let r: Result<usize, diesel::result::Error> = conn.interact(move |c| {
    diesel::insert_into(auth_access_token::table)
      .values(NewAuthAccessToken::new(refresh_token_id, access_token))
      .execute(c)
  }).await.unwrap();

  if r.is_err() {
    return None;
  }

  Some(())
}

/// Deletes obsolete access tokens for a given refresh token.
pub async fn delete_obsolete_access_tokens(conn: DbConn, refresh_token_id: i32) -> Result<usize, diesel::result::Error> {
  conn.interact(move |c| {
    diesel::delete(
      auth_access_token::table
        .filter(auth_access_token::refresh_token_id.eq(refresh_token_id))
    )
      .execute(c)
  }).await.unwrap()
}
