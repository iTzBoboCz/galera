use crate::diesel::QueryDsl;
use crate::diesel::RunQueryDsl;
use crate::schema::user;
use crate::DbConn;
use diesel::select;
use diesel::sql_types::Integer;

use crate::diesel::ExpressionMethods;
use crate::diesel::OptionalExtension;
// use crate::diesel::BoolExpressionMethods;
// use crate::diesel::query_builder::SelectStatement;


/// returns last inserted id
pub async fn get_last_insert_id(conn: &DbConn) -> Option<i32> {
  conn.run(|c| {
    no_arg_sql_function!(last_insert_id, Integer);

    let generated_id: Option<i32> = select(last_insert_id)
      .first(c)
      .optional()
      .unwrap();

    return generated_id;
  }).await
}

/// gets user's ID from username
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

/// gets user's username from ID
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
