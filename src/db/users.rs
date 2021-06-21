use crate::schema::user;
use crate::DbConn;
use diesel::ExpressionMethods;
use diesel::OptionalExtension;
use diesel::QueryDsl;
use diesel::RunQueryDsl;

/// Gets user's ID from username.
/// # Example
/// We're selecting user with username michael.
/// ```
/// let user: Option<i32> = get_user_id(&conn, "michael");
/// ```
pub async fn get_user_id(conn: &DbConn, username: &'static str) -> Option<i32> {
  conn.run(move |c| {
    let user_id: Option<i32> = user::table
      .select(user::id)
      .filter(user::username.eq(username))
      .first(c)
      .optional()
      .unwrap();

    return user_id;
  }).await
}

/// gets user's ID from username
/// # Example
/// We're selecting user with ID 1.
/// ```
/// let user: Option<String> = get_user_username(&conn, 1);
/// ```
pub async fn get_user_username(conn: &DbConn, user_id: i32) -> Option<String> {
  conn.run(move |c| {
    let username: Option<String> = user::table
      .select(user::username)
      .filter(user::id.eq(user_id))
      .first(c)
      .optional()
      .unwrap();

    return username;
  }).await
}
