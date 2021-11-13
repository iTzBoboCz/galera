use crate::{DbConn, db::users::{check_user_login_email, check_user_login_username}, models::User};
use serde::{Serialize, Deserialize};
use sha2::Digest;
use super::token::{Claims, ClaimsEncoded};

/// Used for receiving login data.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct UserLogin {
  username_or_email: String,
  // #[validate(length(min = 8, max = 128))]
  password: String,
}

impl UserLogin {
  /// Checks whether the `username_or_email` field is an email or not.
  fn is_email(&self) -> bool {
    self.username_or_email.contains('@')
  }

  /// Checks the credentials.
  async fn check(&self, conn: &DbConn) -> Option<i32> {
    if self.is_email() {
      return check_user_login_email(conn, self.username_or_email.clone(), self.password.clone()).await;
    } else {
      return check_user_login_username(conn, self.username_or_email.clone(), self.password.clone()).await;
    }
  }

  /// Tries to log the user in.
  pub async fn login(&self, conn: &DbConn) -> Option<Claims> {
    let user_id = self.check(conn).await?;

    let token = Claims::new(user_id);

    // add refresh and access tokens to db
    let refresh_token_id = token.add_refresh_token_to_db(conn).await?;
    token.add_access_token_to_db(conn, refresh_token_id).await?;

    Some(token)
  }

  /// Encrypts the password.
  // TODO: deduplicate later
  pub fn hash_password(mut self) -> Self {
    let mut hasher = sha2::Sha512::new();
    hasher.update(self.password);
    // {:x} means format as hexadecimal
    self.password = format!("{:X}", hasher.finalize());

    self
  }
}

/// Used for sending information about user.
#[derive(Serialize, JsonSchema)]
pub struct UserInfo {
  username: String,
  email: String
}

impl From<User> for UserInfo {
  /// Converts a `User` struct to a `UserInfo`, which is a User struct without an id and a password field.\
  /// # Example
  /// ```
  /// let user = User {
  ///   id: 0,
  ///   username: "John".to_string(),
  ///   email: "john@email.com".to_string(),
  ///   password: "secret".to_string()
  /// };
  ///
  /// let user_info = UserInfo::from(user);
  /// ```
  fn from(user: User) -> UserInfo {
    UserInfo { username: user.username, email: user.email }
  }
}

/// Response when logging in.
#[derive(Serialize, JsonSchema)]
pub struct LoginResponse {
  user_info: UserInfo,
  bearer_token: String,
}

impl LoginResponse {
  pub fn new(claims_encoded: ClaimsEncoded, user_info: UserInfo) -> Self {
    Self {
      user_info,
      bearer_token: claims_encoded.encoded_claims(),
    }
  }
}
