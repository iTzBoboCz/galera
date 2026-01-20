use std::{convert::TryFrom, sync::Arc};
use chrono::Utc;
use serde::{Serialize, Deserialize};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation};
use tracing::error;
use uuid::Uuid;
use crate::{AppState, ConnectionPool, db::{self, tokens::{insert_access_token, insert_refresh_token, insert_session_tokens, select_refresh_token_expiration}, users}};
use crate::DbConn;
use crate::auth::secret::Secret;
use anyhow::{self, Context};
use axum::{http::{StatusCode,Request}, extract::State, response::Response, middleware::Next, body::Body};
use axum_extra::{TypedHeader, headers::{Authorization, authorization}};

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
// #[derive(JsonSchema)]
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
  /// expiration time
  exp: i64,
  /// issued at
  iat: i64,
  /// ID of a user
  pub user_id: i32,
  /// Refresh token - used to refresh access token
  refresh_token: String,
  /// Access token - used to access data
  access_token: String,
}

/// Encoded bearer token
/// # Example
/// decode an encoded bearer token
/// ```
/// let encoded_token = Claims::new(1).encode().unwrap();
///
/// let decoded_token = encoded_token.decode();
/// ```
// #[derive(JsonSchema)]
#[derive(Serialize, Deserialize, Clone)]
pub struct ClaimsEncoded {
  encoded_claims: String,
}

impl ClaimsEncoded {
  /// Returns the encoded token.
  pub fn encoded_claims(&self) -> String {
    self.encoded_claims.clone()
  }

  /// Decodes a bearer token.
  pub fn decode(self) -> Result<TokenData<Claims>, jsonwebtoken::errors::Error> {
    let secret = Secret::read().context("Secret couldn't be read.").unwrap();

    let decoded = jsonwebtoken::decode::<Claims>(self.encoded_claims.as_str(), &DecodingKey::from_secret(secret.as_ref()), &Validation::new(Algorithm::HS512));

    Ok(decoded?)
  }

  pub fn decode_without_validation(self) -> Result<TokenData<Claims>, jsonwebtoken::errors::Error> {
    let secret = Secret::read().context("Secret couldn't be read.").unwrap();

    let mut v = Validation::new(Algorithm::HS512);
    v.validate_exp = false;

    Ok(jsonwebtoken::decode::<Claims>(self.encoded_claims.as_str(), &DecodingKey::from_secret(secret.as_ref()),
    &v)?)
  }
}

impl TryFrom<&str> for Claims {
  type Error = jsonwebtoken::errors::Error;

  /// Tries to convert encoded bearer token presented as a string to a Claims struct.\
  /// Will return error if token can't be decoded.
  /// # Example
  /// ```
  /// let my_bearer_string = "<encoded_bearer>";
  ///
  /// let result = Claims::try_from(my_bearer_string)?;
  /// ```
  fn try_from(token: &str) -> Result<Claims, Self::Error> {
    let encoded = ClaimsEncoded {
      encoded_claims: token.to_owned(),
    };

    Ok(encoded.decode()?.claims)
  }
}

impl Claims {
  /// Checks the exp field of bearer token for its expiration.
  fn is_expired(&self) -> bool {
    let current_time = Utc::now().timestamp();

    self.exp < current_time
  }

  /// Checks whether the refresh token is expired or not.
  pub async fn is_refresh_token_expired(&self, mut conn: DbConn) -> bool {
    let Some(refresh_token_exp) = select_refresh_token_expiration(&mut conn, self.refresh_token.clone()).await else {
      return true;
    };

    let current_time = Utc::now();

    refresh_token_exp.and_utc() < current_time
  }

  /// Checks the validity of a bearer token.
  pub async fn is_valid(&self, pool: ConnectionPool) -> bool {
    // expiration
    !self.is_expired()
    // valid user
    && users::get_user_username(pool.get().await.unwrap(), self.user_id).await.is_some()
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
    if let Err(err) = encoded_claims {
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
      refresh_token: Claims::generate_random_string(),
      access_token: Claims::generate_random_string()
    }
  }

  /// Makes a new token from an old one.
  /// # Example
  /// This will recreate a bearer token for user with ID 1.
  /// ```
  /// let bearer_token = Claims::new(1);
  ///
  /// let new_token = Claims::from_existing(&bearer_token);
  /// ```

  pub fn from_existing(token: &Claims) -> Claims {
    let mut new_token = Claims::new(token.user_id);
    new_token.refresh_token = token.refresh_token.clone();

    new_token
  }

  /// Returns the refresh token.
  pub fn refresh_token(&self) -> String {
    self.refresh_token.clone()
  }

  /// Returns the access token.
  pub fn access_token(&self) -> String {
    self.access_token.clone()
  }

  pub async fn add_session_tokens_to_db(&self, pool: ConnectionPool) -> Result<(i32, i32), diesel::result::Error> {
    insert_session_tokens(pool.get().await.unwrap(), self.user_id, self.refresh_token(), self.access_token()).await
  }

  #[allow(dead_code)]
  /// Adds a new refresh token to the database.
  /// # Example
  /// Adds the `refresh_token` of a bearer token for user with ID 1 to the database.
  /// ```
  /// let bearer_token = Claims::new(1);
  ///
  /// bearer_token.add_refresh_token_to_db(conn)
  /// ```
  pub async fn add_refresh_token_to_db(&self, pool: ConnectionPool) -> Result<i32, diesel::result::Error> {
    insert_refresh_token(pool.get().await.unwrap(), self.user_id, self.refresh_token()).await
  }

  /// Adds a new access token to the database.
  /// # Example
  /// Adds the `access_token` of a bearer token for user with ID 1 to the database.
  /// ```
  /// let bearer_token = Claims::new(1);
  ///
  /// let refresh_token_id = bearer_token.add_refresh_token_to_db(conn).await?;
  /// bearer_token.add_access_token_to_db(conn, refresh_token_id).await?;
  /// ```
  pub async fn add_access_token_to_db(&self, pool: ConnectionPool, refresh_token_id: i32) -> Result<i32, diesel::result::Error> {
    insert_access_token(pool.get().await.unwrap(), refresh_token_id, self.access_token()).await
  }

  /// Deletes obsolete access tokens for a given refresh token ID from the database.
  /// # Example
  /// This will create a bearer token and refresh it.
  /// ```
  /// let bearer_token = Claims::new(1);
  ///
  /// // add refresh and access tokens to db
  /// let refresh_token_id = bearer_token.add_refresh_token_to_db(conn).await?;
  /// bearer_token.add_access_token_to_db(conn, refresh_token_id).await?;
  ///
  /// // create a new token from the previous one; only the refresh_token will be the same
  /// let new_token = Claims::from_existing(&bearer_token);
  ///
  /// // remove obsolete access tokens
  /// Claims::delete_obsolete_access_tokens(&conn, refresh_token_id).await;
  ///
  /// // add a new access token
  /// new_token.add_access_token_to_db(conn, refresh_token_id).await?;
  /// ```
  pub async fn delete_obsolete_access_tokens(conn: DbConn, refresh_token_id: i32) -> Option<()> {
    if db::tokens::delete_obsolete_access_tokens(conn, refresh_token_id).await.is_err() { return None; };

    Some(())
  }

  /// Generates a new random string.
  fn generate_random_string() -> String {
    Uuid::new_v4().to_string()
  }
}

/// Auth middleware.
pub async fn auth(State(AppState { pool,.. }): State<AppState>, TypedHeader(Authorization(bearer)): TypedHeader<Authorization<authorization::Bearer>>, mut req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
  let bearer_token_decoded = Claims::try_from(bearer.token());

  if let Ok(claims) = bearer_token_decoded {
    if claims.is_valid(pool).await {
      // insert the current user into a request extension so the handler can
      // extract it
      req.extensions_mut().insert(Arc::new(claims));
      return Ok(next.run(req).await)
    };

    error!("Bearer token is invalid.");
    return Err(StatusCode::UNAUTHORIZED);
  }

  let error_status = match bearer_token_decoded.unwrap_err().kind() {
    jsonwebtoken::errors::ErrorKind::ExpiredSignature => StatusCode::UNAUTHORIZED,
    _ => StatusCode::UNPROCESSABLE_ENTITY
  };

  // TODO: check refresh token validity and if access token still exists (one device => people cant steal it well)

  Err(error_status)
}

use super::shared_album_link::shared_album_link;

pub async fn mixed_auth(State(state): State<AppState>, bearer: Option<TypedHeader<Authorization<authorization::Bearer>>>, special_auth: Option<TypedHeader<Authorization<authorization::Basic>>>, req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
  if let Some(bearer) = bearer {
    if let Ok(result) = auth(State(state), bearer, req, next).await {
      return Ok(result);
    }
  } else if let Some(special_auth) = special_auth {
    if let Ok(result) = shared_album_link(State(state), special_auth, req, next).await {
      return Ok(result);
    }
  }

  Err(StatusCode::UNAUTHORIZED)
}

// impl<'a, 'r> OpenApiFromRequest<'a> for Claims {
//   fn from_request_input(
//     _gen: &mut OpenApiGenerator,
//     _name: String,
//     _required: bool,
//   ) -> rocket_okapi::Result<RequestHeaderInput> {
//     let mut security_req = SecurityRequirement::new();
//     // each security requirement needs a specific key in the openapi docs
//     security_req.insert("BearerAuth".into(), Vec::new());

//     // The scheme for the security needs to be defined as well
//     // https://swagger.io/docs/specification/authentication/basic-authentication/
//     let security_scheme = SecurityScheme {
//       description: Some("requires a bearer token to access".into()),
//       // this will show where and under which name the value will be found in the HTTP header
//       // in this case, the header key x-api-key will be searched
//       // other alternatives are "query", "cookie" according to the openapi specs.
//       // [link](https://swagger.io/specification/#security-scheme-object)
//       // which also is where you can find examples of how to create a JWT scheme for example
//       data: SecuritySchemeData::Http {
//         scheme: String::from("bearer"),
//         bearer_format: Some(String::from("JWT")),
//       },
//       extensions: Object::default(),
//     };

//     Ok(RequestHeaderInput::Security(
//       // scheme identifier is the keyvalue under which this security_scheme will be filed in
//       // the openapi.json file
//       "BearerAuth".to_owned(),
//       security_scheme,
//       security_req,
//     ))
//   }
// }

// impl<'a, 'r> OpenApiFromRequest<'a> for DbConn {
//   fn from_request_input(
//     _gen: &mut OpenApiGenerator,
//     _name: String,
//     required: bool,
//   ) -> rocket_okapi::Result<RequestHeaderInput> {
//     Ok(RequestHeaderInput::None)
//   }
// }

// /// Returns an empty, default `Response`. Always returns `Ok`.
// /// Defines the possible response for this request guard
// impl<'a, 'r: 'a> rocket::response::Responder<'a, 'r> for Claims {
//   fn respond_to(self, _: &Request<'_>) -> rocket::response::Result<'static> {
//     Ok(Response::new())
//   }
// }

// impl<'a, 'r: 'a> rocket::response::Responder<'a, 'r> for DbConn {
//   fn respond_to(self, _: &Request<'_>) -> rocket::response::Result<'static> {
//     Ok(Response::new())
//   }
// }

// /// Defines the possible responses for this request guard for the openapi docs (not used yet)
// impl<'a, 'r: 'a> OpenApiResponder<'a, 'r> for Claims {
//   fn responses(_: &mut OpenApiGenerator) -> rocket_okapi::Result<Responses> {
//     let responses = Responses::default();
//     Ok(responses)
//   }
// }
