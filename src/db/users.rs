use crate::models::NewUser;
use crate::schema::user;
use crate::DbConn;
use diesel::ExpressionMethods;
use diesel::OptionalExtension;
use diesel::QueryDsl;
use diesel::RunQueryDsl;

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
