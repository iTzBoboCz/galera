use crate::diesel::QueryDsl;
use crate::diesel::RunQueryDsl;
use crate::schema::user;
use crate::Pool;
use diesel::select;
use diesel::sql_types::Integer;

use crate::diesel::ExpressionMethods;
use crate::diesel::OptionalExtension;
// use crate::diesel::BoolExpressionMethods;
// use crate::diesel::query_builder::SelectStatement;


/// returns last inserted id
pub fn get_last_insert_id(pool: Pool) -> Option<i32> {
  no_arg_sql_function!(last_insert_id, Integer);
  let generated_id: Option<i32> = select(last_insert_id)
    .first(&pool.get().unwrap())
    .optional()
    .unwrap();
  return generated_id;
}

/// gets user's ID from username
pub fn get_user_id(pool: Pool, username: &str) -> Option<i32> {
  let user_id: Option<i32> = user::table
    .select(user::id)
    .filter(user::username.eq(username))
    .first(&pool.get().unwrap())
    .optional()
    .unwrap();

  return user_id;
}

/// gets user's username from ID
pub fn get_user_username(pool: Pool, user_id: i32) -> Option<String> {
  let username: Option<String> = user::table
    .select(user::username)
    .filter(user::id.eq(user_id))
    .first(&pool.get().unwrap())
    .optional()
    .unwrap();

  return username;
}
