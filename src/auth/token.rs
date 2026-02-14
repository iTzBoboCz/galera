use std::{convert::TryFrom, sync::Arc};
use chrono::Utc;
use serde::{Serialize, Deserialize};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, TokenData, Validation};
use tracing::error;
use uuid::Uuid;
use utoipa::ToSchema;
use crate::{AppState, ConnectionPool, db::{tokens::{insert_access_token, insert_session_tokens, select_refresh_token_expiration}, users}, instance_uuid};
use crate::DbConn;
use crate::auth::secret::Secret;
use anyhow::{self, Context};
use axum::{http::{StatusCode,Request}, extract::State, response::Response, middleware::Next, body::Body};
use axum_extra::{TypedHeader, headers::{Authorization, authorization}};

#[derive(Debug, Serialize, Deserialize)]
enum Roles {
  Admin,
  User
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
  /// Issuer
  iss: String,
  /// Subject - UUID of a user
  pub sub: String,
  /// Audience
  aud: String,
  /// expiration time
  exp: i64,
  /// not before
  nbf: i64,
  /// issued at
  iat: i64,
  /// JWT ID
  jti: String,
  /// ID of a user (temporarily for compatibility, will be removed soon)
  pub user_id: i32,
  roles: Vec<Roles>
}

/// Encoded bearer token
/// # Example
/// decode an encoded bearer token
/// ```
/// let encoded_token = Claims::new(1).encode().unwrap();
///
/// let decoded_token = encoded_token.decode();
/// ```
#[derive(Serialize, Deserialize, Clone, ToSchema)]
pub struct ClaimsEncoded {
  pub encoded_claims: String,
}

impl ClaimsEncoded {
  /// Returns the encoded token.
  pub fn encoded_claims(&self) -> String {
    self.encoded_claims.clone()
  }

  /// Decodes a bearer token.
  pub fn decode(self) -> Result<TokenData<Claims>, jsonwebtoken::errors::Error> {
    let secret = Secret::read().context("Secret couldn't be read.").unwrap();

    let mut v = Validation::new(Algorithm::HS512);
    v.set_audience(&["urn:galera:api"]);

    let decoded = jsonwebtoken::decode::<Claims>(self.encoded_claims.as_str(), &DecodingKey::from_secret(secret.as_ref()), &v);

    Ok(decoded?)
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
    let encoded = ClaimsEncoded { encoded_claims: token.to_owned() };

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
  pub async fn is_refresh_token_expired(mut conn: DbConn, refresh_token: String) -> bool {
    let Some(refresh_token_exp) = select_refresh_token_expiration(&mut conn, refresh_token).await else {
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
  pub fn new(user_id: i32, sub: String) -> Claims {
    let current_time = Utc::now().timestamp();

    // 15 mins in seconds
    let expiraton_time = 900;

    Claims {
      iss: format!("urn:galera:instance:{}", instance_uuid().unwrap()),
      sub,
      aud: "urn:galera:api".into(),
      exp: current_time + expiraton_time,
      nbf: current_time,
      iat: current_time,
      jti: Claims::generate_random_string(),
      user_id,
      roles: vec![Roles::User]
    }
  }

  /// Returns the access token.
  pub fn access_token(&self) -> String {
    self.jti.clone()
  }

  pub async fn add_session_tokens_to_db(&self, pool: ConnectionPool, refresh_token: String) -> Result<(i32, i32), diesel::result::Error> {
    insert_session_tokens(pool.get().await.unwrap(), self.user_id, refresh_token, self.access_token()).await
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
