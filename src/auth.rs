use std::{fs::{self, File}, io::Read};
use rand::{thread_rng, Rng, distributions::Alphanumeric};
use chrono::Utc;
use rocket::request::{self, FromRequest, Request, Outcome };
use serde::{Serialize, Deserialize};
use jsonwebtoken::{Header, Algorithm, Validation, EncodingKey, DecodingKey};
use uuid::Uuid;

/// Request guard that represents a user
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
  // aud: String,         // Optional. Audience
  exp: usize,          // Required (validate_exp defaults to true in validation). Expiration time (as UTC timestamp)
  // iat: usize,          // Optional. Issued at (as UTC timestamp)
  // iss: String,         // Optional. Issuer
  // nbf: usize,          // Optional. Not Before (as UTC timestamp)
  // sub: String,         // Optional. Subject (whom token refers to)
  // refresh_token: i32,
}

impl Claims {
  fn is_expired(&self) -> bool {
    let current_time = Utc::now().timestamp();

    // 15 mins in seconds
    let expiraton_time = 900;

    self.exp as i64 > (current_time + expiraton_time)
  }
  fn get_exp(&self) -> usize {
    self.exp
  }
}

// pub fn test() {
//   let mut header = Header::new(Algorithm::HS512);

//   let token = jsonwebtoken::encode(&header, &my_claims, &EncodingKey::from_secret("secret".as_ref()))?;


//   let mut header = Header::new(Algorithm::HS512);
//   header.kid = Some("blabla".to_owned());
//   let token = jsonwebtoken::encode(&header, &my_claims, &EncodingKey::from_secret("secret".as_ref()))?;

// }

// TODO:
// https://rocket.rs/v0.5-rc/guide/state/#within-guards
// https://crates.io/crates/rocket_okapi_fork/versions
// https://medium.com/@james_32022/authentication-in-rocket-feb4f7223254
// https://github.com/magiclen/rocket-jwt-authorization/blob/master/src/lib.rs
// https://curity.io/resources/learn/jwt-best-practices/
// https://blog.logrocket.com/jwt-authentication-in-rust/
// https://security.stackexchange.com/questions/119371/is-refreshing-an-expired-jwt-token-a-good-strategy
// https://github.com/GREsau/okapi/pull/47/files#diff-e022d975abdfa9c7a10536c5107e3f660d3ea50534fd8f486fde8b14972b447f
// https://github.com/SakaDream/rocket-rest-api-with-jwt/blob/master/src/jwt.rs
// https://stackoverflow.com/questions/26340275/where-to-save-a-jwt-in-a-browser-based-application-and-how-to-use-it/40376819#40376819

// SOLUTION: https://stackoverflow.com/a/42764942
#[rocket::async_trait]
impl<'r> FromRequest<'r> for Claims {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
      // error!("{:?}", request);
      // fn is_valid() {
      //   self.exp
      //   // 1. expiration
      //   // 2. check db if token is valid
      // }
      error!("{}", request);
      let outcome = rocket::outcome::try_outcome!(request.guard::<Claims>().await);

      if outcome.is_expired() {
        Outcome::Forward(())
      } else {
        Outcome::Success(outcome)
      }

      // error!("{:?}", outcome.get_exp());
    }
}

impl Claims {
  /// Generates new refresh token
  fn generate_refresh_token() -> String {
    return Uuid::new_v4().to_string();
  }


}

pub fn generate_token() -> String {
  let current_time = Utc::now().timestamp();

  // 15 mins in seconds
  let expiraton_time = 900;

  let claims = Claims {
    exp: (current_time + expiraton_time) as usize,
  };

  let header = Header::new(Algorithm::HS512);

  let secret = read_secret();

  jsonwebtoken::encode(&header, &claims, &EncodingKey::from_secret(secret.unwrap().as_bytes())).unwrap()
}

// https://stackoverflow.com/a/65478580
pub fn generate_secret() -> String {
  let mut rng = thread_rng();

  let range = rng.gen_range(256..512);

  String::from_utf8(
    rng.sample_iter(&Alphanumeric)
    .take(range)
    .collect::<Vec<_>>(),
  ).unwrap()
}

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

// pub fn decode() -> String {

// }

// pub fn encode() -> String {

// }
