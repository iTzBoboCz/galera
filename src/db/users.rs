use crate::models::{NewUser, User};
use crate::schema::user;
use crate::DbConn;
use diesel::BoolExpressionMethods;
use diesel::ExpressionMethods;
use diesel::OptionalExtension;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use diesel::Table;
use tracing::error;

/// Inserts a new user.
/// # Example
/// ```
/// let user = NewUser {
///   username: String::from("foo"),
///   email: String::from("foo@bar.foo"),
///   password: String::from("bar")
/// };
/// insert_user(&conn, user);
/// ```
pub async fn insert_user(conn: DbConn, user: NewUser) -> usize {
  conn.interact(move |c| {
    diesel::insert_into(user::table)
      .values(user.clone())
      .execute(c)
      .unwrap_or_else(|_| panic!("Error creating user {}", user.username))
  }).await.unwrap()
}

/// Checks whether user is unique in database.
/// # Example
/// This will add a new user only if it doesn't already exist.
/// ```
/// let user = NewUser {
///   username: String::from("foo"),
///   email: String::from("foo@bar.foo"),
///   password: String::from("bar")
/// };
/// if is_user_unique(&conn, user) {
///   insert_user(&conn, user);
/// }
/// ```
pub async fn is_user_unique(conn: DbConn, user: NewUser) -> bool {
  conn.interact(move |c| {
    let user_id: Option<i32> = user::table
      .select(user::id)
      .filter(user::username.eq(user.username))
      .or_filter(user::email.eq(user.email))
      .first(c)
      .optional()
      .unwrap();

    if user_id.is_some() { return false }

    true
  }).await.unwrap()
}

#[allow(dead_code)]
/// Gets user's ID from username.
/// # Example
/// We're selecting user with username michael.
/// ```
/// let user: Option<i32> = get_user_id(&conn, String::from("michael"));
/// ```
pub async fn get_user_id(conn: DbConn, username: String) -> Option<i32> {
  conn.interact(move |c| {
    user::table
      .select(user::id)
      .filter(user::username.eq(username))
      .first(c)
      .optional()
      .unwrap()
  }).await.unwrap()
}

/// Tries to select a user by its ID.
/// # Example
/// We're selecting the username of a user with ID 1.
/// ```
/// let username: Option<String> = get_user_username(&conn, 1);
/// ```
pub async fn get_user_username(conn: DbConn, user_id: i32) -> Option<String> {
  conn.interact(move |c| {
    user::table
      .select(user::username)
      .filter(user::id.eq(user_id))
      .first(c)
      .optional()
      .unwrap()
  }).await.unwrap()
}

/// Tries to select a user by its ID.
pub async fn get_user_by_id(conn: DbConn, user_id: i32) -> Option<User> {
  conn.interact(move |c| {
    user::table
      .select(user::table::all_columns())
      .filter(user::id.eq(user_id))
      .first::<User>(c)
      .optional()
      .unwrap()
  }).await.unwrap()
}

/// Tries to select a user from a given email.
pub async fn get_user_by_email(conn: DbConn, email: String) -> Result<Option<User>, diesel::result::Error> {
  let result = conn
    .interact(move |c| {
      user::table
        .select(user::table::all_columns())
        .filter(user::email.eq(email))
        .first::<User>(c)
        .optional()
    })
    .await
    .map_err(|e| {
      error!("DB interact failed in get_user_by_email: {e}");
      diesel::result::Error::DatabaseError(
        diesel::result::DatabaseErrorKind::Unknown,
        Box::new(format!("interact failed: {e}")),
      )
    })??;

  Ok(result)
}

/// Checks the database for a combination of a specified username and password.
pub async fn check_user_login_username(conn: DbConn, username: String, password: String) -> Option<i32> {
  conn.interact(move |c| {
    user::table
      .select(user::id)
      .filter(user::username.eq(username).and(user::password.eq(password)))
      .first(c)
      .optional()
      .unwrap()
  }).await.unwrap()
}

/// Checks the database for a combination of a specified email and password.
pub async fn check_user_login_email(conn: DbConn, email: String, password: String) -> Option<i32> {
  conn.interact(move |c| {
    user::table
      .select(user::id)
      .filter(user::email.eq(email).and(user::password.eq(password)))
      .first(c)
      .optional()
      .unwrap()
  }).await.unwrap()
}
