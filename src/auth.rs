use std::fs::{self, File};
use okapi::openapi3::{
  Object, Responses, SecurityRequirement,
  SecurityScheme, SecuritySchemeData,
};
use rand::{Rng, distributions::Alphanumeric, thread_rng};
use chrono::Utc;
use rocket::{
  Response,
  request::{FromRequest, Request, Outcome},
};
use serde::{Serialize, Deserialize};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation};
use uuid::Uuid;
use rocket::http::Status;
use crate::db::users;
use crate::DbConn;

use rocket_okapi::{
  gen::OpenApiGenerator,
  request::{OpenApiFromRequest, RequestHeaderInput},
  response::OpenApiResponder,
};
use anyhow;

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

    self.exp < (current_time + expiraton_time)
  }

  /// Checks the validity of a bearer token.
  pub async fn is_valid(&self, conn: DbConn) -> bool {
    // expiration
    !self.is_expired()
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
    // error!("headers: {:?}", headers);
    if headers.is_empty() {
      error!("Headers are not valid.");
      return Outcome::Failure((Status::Unauthorized, ()));
    }
    let authorization_header: Vec<&str> = headers
      .get("authorization")
      .filter(|r| !r.is_empty())
      .collect();

    if authorization_header.is_empty() {
      error!("Authorization header is empty!");
      return Outcome::Failure((Status::Unauthorized, ()));
    }

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

impl<'a, 'r> OpenApiFromRequest<'a> for Claims {
  fn from_request_input(
    _gen: &mut OpenApiGenerator,
    _name: String,
    _required: bool,
  ) -> rocket_okapi::Result<RequestHeaderInput> {
    let mut security_req = SecurityRequirement::new();
    // each security requirement needs a specific key in the openapi docs
    security_req.insert("BearerAuth".into(), Vec::new());

    // The scheme for the security needs to be defined as well
    // https://swagger.io/docs/specification/authentication/basic-authentication/
    let security_scheme = SecurityScheme {
      description: Some("requires a bearer token to access".into()),
      // this will show where and under which name the value will be found in the HTTP header
      // in this case, the header key x-api-key will be searched
      // other alternatives are "query", "cookie" according to the openapi specs.
      // [link](https://swagger.io/specification/#security-scheme-object)
      // which also is where you can find examples of how to create a JWT scheme for example
      data: SecuritySchemeData::Http {
        scheme: String::from("bearer"),
        bearer_format: Some(String::from("JWT")),
      },
      extensions: Object::default(),
    };

    Ok(RequestHeaderInput::Security(
      // scheme identifier is the keyvalue under which this security_scheme will be filed in
      // the openapi.json file
      "BearerAuth".to_owned(),
      security_scheme,
      security_req,
    ))
  }
}

impl<'a, 'r> OpenApiFromRequest<'a> for DbConn {
  fn from_request_input(
    _gen: &mut OpenApiGenerator,
    _name: String,
    required: bool,
  ) -> rocket_okapi::Result<RequestHeaderInput> {
    Ok(RequestHeaderInput::None)
  }
}

/// Returns an empty, default `Response`. Always returns `Ok`.
/// Defines the possible response for this request guard
impl<'a, 'r: 'a> rocket::response::Responder<'a, 'r> for Claims {
  fn respond_to(self, _: &rocket::request::Request<'_>) -> rocket::response::Result<'static> {
    Ok(Response::new())
  }
}

impl<'a, 'r: 'a> rocket::response::Responder<'a, 'r> for DbConn {
  fn respond_to(self, _: &rocket::request::Request<'_>) -> rocket::response::Result<'static> {
    Ok(Response::new())
  }
}

/// Defines the possible responses for this request guard for the openapi docs (not used yet)
impl<'a, 'r: 'a> OpenApiResponder<'a, 'r> for Claims {
  fn responses(_: &mut OpenApiGenerator) -> rocket_okapi::Result<Responses> {
    let responses = Responses::default();
    Ok(responses)
  }
}

/// Generates a new refresh token.
fn generate_refresh_token() -> String {
  return Uuid::new_v4().to_string();
}

/// Decodes a bearer token.
fn decode_bearer_token(token: &str) -> anyhow::Result<TokenData<Claims>> {
  Ok(jsonwebtoken::decode::<Claims>(token, &DecodingKey::from_secret(Secret::read()?.as_ref()), &Validation::new(Algorithm::HS512))?)
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
    refresh_token: generate_refresh_token(),
  };

  let header = Header::new(Algorithm::HS512);

  let secret = Secret::read();

  // TODO: better error handling
  if secret.is_err() { error!("Secret couldn't be read."); }

  jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret.unwrap().as_bytes())).unwrap()
}

pub struct Secret {
  key: String,
}

impl Secret {
  /// Generates a new secret.
  /// # Example
  /// ```
  /// let my_secret_string = Secret::generate();
  /// ```
  fn generate() -> String {
    let mut rng = thread_rng();

    let range = rng.gen_range(256..512);

    String::from_utf8(
      rng
        .sample_iter(&Alphanumeric)
        .take(range)
        .collect::<Vec<_>>(),
    )
    .unwrap()
  }

  /// Reads content of a secret.key file.
  // TODO: check for write and read permissions
  pub fn read() -> Result<String, std::io::Error> {
    let path = "secret.key";
    fs::read_to_string(path)
  }

  /// Writes a secret to the secret.key file.
  /// # Example
  /// ```
  /// // creates a new secret
  /// let my_secret = Secret::new();
  ///
  /// // writes it to the disk
  /// my_secret.write();
  /// ```
  pub fn write(self) -> std::io::Result<()> {
    let path = "secret.key";
    fs::write(path, self.key)
  }

  /// Creates a new secret
  /// # Example
  /// ```
  /// let my_secret = Secret::new();
  /// ```
  pub fn new() -> Secret {
    Secret {
      key: Secret::generate()
    }
  }
}
