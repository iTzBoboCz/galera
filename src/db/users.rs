use crate::models::NewUser;
use crate::models::User;
use crate::schema::user;
use crate::DbConn;
use diesel::BoolExpressionMethods;
use diesel::ExpressionMethods;
use diesel::OptionalExtension;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use diesel::Table;

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
pub async fn insert_user(conn: &DbConn, user: NewUser) -> usize {
  conn.run(move |c| {
    let insert = diesel::insert_into(user::table)
      .values(user.clone())
      .execute(c)
      .expect(format!("Error creating user {}", user.username).as_str());

    return insert;
  }).await
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
pub async fn is_user_unique(conn: &DbConn, user: NewUser) -> bool {
  conn.run(move |c| {
    let user_id: Option<i32> = user::table
      .select(user::id)
      .filter(user::username.eq(user.username))
      .or_filter(user::email.eq(user.email))
      .first(c)
      .optional()
      .unwrap();

    if user_id.is_some() { return false }

    true
  }).await
}

/// Gets user's ID from username.
/// # Example
/// We're selecting user with username michael.
/// ```
/// let user: Option<i32> = get_user_id(&conn, String::from("michael"));
/// ```
pub async fn get_user_id(conn: &DbConn, username: String) -> Option<i32> {
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

/// Tries to select a user by its ID.
/// # Example
/// We're selecting the username of a user with ID 1.
/// ```
/// let username: Option<String> = get_user_username(&conn, 1);
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

/// Tries to select a user by its ID.
pub async fn get_user_by_id(conn: &DbConn, user_id: i32) -> Option<User> {
  conn.run(move |c| {
    user::table
      .select(user::table::all_columns())
      .filter(user::id.eq(user_id))
      .first::<User>(c)
      .optional()
      .unwrap()
  }).await
}

/// Tries to select a user_id from a given email.
pub async fn get_user_id_email(conn: &DbConn, email: String) -> Option<i32> {
  conn.run(move |c| {
    user::table
      .select(user::id)
      .filter(user::email.eq(email))
      .first(c)
      .optional()
      .unwrap()
  }).await
}

/// Checks the database for a combination of a specified username and password.
pub async fn check_user_login_username(conn: &DbConn, username: String, password: String) -> Option<i32> {
  conn.run(move |c| {
    user::table
      .select(user::id)
      .filter(user::username.eq(username).and(user::password.eq(password)))
      .first(c)
      .optional()
      .unwrap()
  }).await
}

/// Checks the database for a combination of a specified email and password.
pub async fn check_user_login_email(conn: &DbConn, email: String, password: String) -> Option<i32> {
  conn.run(move |c| {
    user::table
      .select(user::id)
      .filter(user::email.eq(email).and(user::password.eq(password)))
      .first(c)
      .optional()
      .unwrap()
  }).await
}
