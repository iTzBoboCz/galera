use std::fs::{self, File};
use rand::{Rng, distributions::Alphanumeric, thread_rng};
use chrono::Utc;
use rocket::request::{ FromRequest, Request, Outcome };
use serde::{Serialize, Deserialize};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation};
use uuid::Uuid;
use rocket::http::Status;
use crate::db::users;
use crate::DbConn;

/// Request guard
/// # Example
/// Only authenticated users will be able to access data on this endpoint.
/// ```
/// #[get("/data")]
/// pub async fn get_data(claims: Claims, conn: DbConn) -> Json<Vec<Data>> {
///   Json(db::request_data(&conn).await)
/// }
/// ```
/// for more information, see [Rocket documentation](https://rocket.rs/v0.5-rc/guide/requests/#request-guards).
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
  /// expiration time
  exp: i64,
  /// issued at
  iat: i64,
  /// ID of a user
  pub user_id: i32,
  /// Refresh token
  refresh_token: String,
}

impl Claims {
  /// Checks whether the bearer token is expired or not.
  fn is_expired(&self) -> bool {
    let current_time = Utc::now().timestamp();

    // 15 mins in seconds
    let expiraton_time = 900;

    self.exp as i64 > (current_time + expiraton_time)
  }

  /// Checks the validity of a bearer token.
  async fn is_valid(&self, conn: DbConn) -> bool {
    // expiration
    self.is_expired()
    // valid user
    && users::get_user_username(&conn, self.user_id.clone()).await.is_some()
    // TODO: other checks
    // valid refresh_token
    // && db::users::(&conn, user_id,)
  }
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Claims {
  type Error = ();

  /// Implements Request guard for Claims.
  async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
    let conn = request.guard::<DbConn>().await.unwrap();

    let headers = request.headers();
    if headers.is_empty() { return Outcome::Failure((Status::Unauthorized, ())); }
    let authorization_header: Vec<&str> = headers.get("authorization")
      .filter(|r| !r.is_empty())
      .collect();

    if authorization_header.is_empty() { return Outcome::Failure((Status::Unauthorized, ())); }

    let bearer_token_encoded: &str = authorization_header[0][6..authorization_header[0].len()].trim();
    let bearer_token_decoded = decode_bearer_token(bearer_token_encoded);

    if bearer_token_decoded.is_ok() {
      let claims = bearer_token_decoded.unwrap().claims;

      if claims.is_valid(conn).await { return Outcome::Success(claims) };
    }

    error!("Bearer token is invalid.");
    Outcome::Failure((Status::Unauthorized, ()))
  }
}

/// Generates a new refresh token.
fn generate_refresh_token() -> String {
  return Uuid::new_v4().to_string();
}

/// Decodes a bearer token.
fn decode_bearer_token(token: &str) -> Result<TokenData<Claims>, jsonwebtoken::errors::Error> {
  jsonwebtoken::decode::<Claims>(token, &DecodingKey::from_secret(read_secret().unwrap().as_ref()), &Validation::new(Algorithm::HS512))
}

/// Generates a new bearer token.
/// # Example
/// This will generate a new bearer token for user with ID 1.
/// ```
/// let new_bearer_token = generate_token(1);
/// ```
pub fn generate_token(user_id: i32) -> String {
  let current_time = Utc::now().timestamp();

  // 15 mins in seconds
  let expiraton_time = 900;

  let claims = Claims {
    exp: current_time + expiraton_time,
    iat: current_time,
    user_id,
    refresh_token: generate_refresh_token()
  };

  let header = Header::new(Algorithm::HS512);

  let secret = read_secret();

  // TODO: better error handling
  if secret.is_none() { error!("Secret couldn't be read."); }

  jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret.unwrap().as_bytes())).unwrap()
}

/// Generates a new secret.
pub fn generate_secret() -> String {
  let mut rng = thread_rng();

  let range = rng.gen_range(256..512);

  String::from_utf8(
    rng.sample_iter(&Alphanumeric)
    .take(range)
    .collect::<Vec<_>>(),
  ).unwrap()
}

/// Reads content of a secret.key file.\
/// If secret.key doesn't exist, it will be created.
// TODO: check for write and read permissions
pub fn read_secret() -> Option<String> {
  let path = "secret.key";
  let file = File::open(path);

  // generate new secret if the file doesn't exist
  if file.is_err() {
    let secret = generate_secret();

    let result = fs::write(path, secret);

    if result.is_err() {
      return None;
    }
  }

  match fs::read_to_string(path) {
    Ok(result) => Some(result),
    Err(_) => None,
  }
}
