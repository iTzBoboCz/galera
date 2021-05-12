use crate::diesel::QueryDsl;
use crate::diesel::RunQueryDsl;
use crate::schema::user;
use crate::Pool;
use diesel::select;
use diesel::sql_types::Integer;

use crate::diesel::ExpressionMethods;

pub fn get_last_insert_id(pool: Pool) -> i32 {
  no_arg_sql_function!(last_insert_id, Integer);
  let generated_id: i32 = select(last_insert_id).first(&pool.get().unwrap()).unwrap();
  return generated_id;
}

pub fn get_user_id(pool: Pool, username: &str) -> i32 {
  let user_id: i32 = user::table
    .select(user::id)
    .filter(user::username.eq(username))
    .first(&pool.get().unwrap())
    .unwrap();

  return user_id;
}

pub fn get_user_username(pool: Pool, user_id: i32) -> String {
  let username: String = user::table
    .select(user::username)
    .filter(user::id.eq(user_id))
    .first(&pool.get().unwrap())
    .unwrap();

  return username;
}
