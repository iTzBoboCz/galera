use std::convert::TryFrom;
use okapi::openapi3::{
  Object, Responses, SecurityRequirement,
  SecurityScheme, SecuritySchemeData,
};
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
use crate::auth::secret::Secret;

use rocket_okapi::{
  gen::OpenApiGenerator,
  request::{OpenApiFromRequest, RequestHeaderInput},
  response::OpenApiResponder,
};
use anyhow::{self, Context};

/// Bearer token\
/// used as a Request guard
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

/// Encoded bearer token
/// # Example
/// decode an encoded bearer token
/// ```
/// let encoded_token = Claims::new(1).encode().unwrap();
///
/// let decoded_token = encoded_token.decode();
/// ```
pub struct ClaimsEncoded {
  encoded_claims: String,
}

impl ClaimsEncoded {
  /// Decodes a bearer token.
  pub fn decode(self) -> anyhow::Result<TokenData<Claims>> {
    let secret = Secret::read().context("Secret couldn't be read.")?;

    let decoded = jsonwebtoken::decode::<Claims>(self.encoded_claims.as_str(), &DecodingKey::from_secret(secret.as_ref()), &Validation::new(Algorithm::HS512));

    // TODO: better error messages
    if decoded.is_err() {
        let err =  decoded.unwrap_err();
        let context = format!("Decoding went wrong. {}.", err);
        return Err(anyhow::Error::new( err).context(context));
    }
    Ok(decoded.unwrap())
  }
}

impl TryFrom<&str> for Claims {
  type Error = anyhow::Error;

  /// Tries to convert encoded bearer token presented as a string to a Claims struct.\
  /// Will return error if token can't be decoded.
  /// # Example
  /// ```
  /// let my_bearer_string = "<encoded_bearer>";
  ///
  /// let result = Claims::try_from(my_bearer_string)?;
  /// ```
  fn try_from(token: &str) -> anyhow::Result<Claims> {
    let encoded = ClaimsEncoded {
      encoded_claims: token.to_owned(),
    };

    Ok(encoded.decode()?.claims)
  }
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

  /// Encodes a bearer token.
  /// # Example
  /// ```
  /// let token = Claims::new(1).encode();
  /// ```
  pub fn encode(self) -> anyhow::Result<ClaimsEncoded> {
    let header = Header::new(Algorithm::HS512);
    let secret = Secret::read()?;

    let encoded_claims = jsonwebtoken::encode(&header, &self, &EncodingKey::from_secret(secret.as_bytes()));

    // TODO: better error messages
    if encoded_claims.is_err() {
        let err =  encoded_claims.unwrap_err();
        let context = format!("Encoding went wrong. {}.", err);
        return Err(anyhow::Error::new( err).context(context));
    }
    Ok(ClaimsEncoded { encoded_claims: encoded_claims.unwrap() })
  }

  /// Generates a new bearer token.
  /// # Example
  /// This will generate a new bearer token for user with ID 1.
  /// ```
  /// let new_bearer_token = Claims::new(1);
  /// ```
  pub fn new(user_id: i32) -> Claims {
    let current_time = Utc::now().timestamp();

    // 15 mins in seconds
    let expiraton_time = 900;

    Claims {
      exp: current_time + expiraton_time,
      iat: current_time,
      user_id,
      refresh_token: Claims::generate_refresh_token(),
    }
  }

  /// Generates a new refresh token.
  fn generate_refresh_token() -> String {
    return Uuid::new_v4().to_string();
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
    let bearer_token_decoded = Claims::try_from(bearer_token_encoded);

    if bearer_token_decoded.is_ok() {
      let claims = bearer_token_decoded.unwrap();

      if claims.is_valid(conn).await { return Outcome::Success(claims) };

      error!("Bearer token is invalid.");
      return Outcome::Failure((Status::Unauthorized, ()));
    }

    error!("{}", bearer_token_decoded.unwrap_err());
    Outcome::Failure((Status::InternalServerError, ()))
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
