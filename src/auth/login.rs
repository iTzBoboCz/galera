use crate::{db::users::{check_user_login_email, check_user_login_username}, models::User, ConnectionPool};
use serde::{Serialize, Deserialize};
use sha2::Digest;
use super::token::{Claims, ClaimsEncoded};

/// Used for receiving login data.
// #[derive(JsonSchema)]
#[derive(Debug, Serialize, Deserialize)]
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
  async fn check(&self, pool: ConnectionPool) -> Option<i32> {
    if self.is_email() {
      return check_user_login_email(pool.get().await.unwrap(), self.username_or_email.clone(), self.password.clone()).await;
    } else {
      return check_user_login_username(pool.get().await.unwrap(), self.username_or_email.clone(), self.password.clone()).await;
    }
  }

  /// Tries to log the user in.
  pub async fn login(&self, pool: ConnectionPool) -> Option<Claims> {
    let user_id = self.check(pool.clone()).await?;

    let token = Claims::new(user_id);

    // add refresh and access tokens to db
    token.add_session_tokens_to_db(pool).await.ok()?;

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
// #[derive(JsonSchema)]
#[derive(Serialize)]
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
// #[derive(JsonSchema)]
#[derive(Serialize)]
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
