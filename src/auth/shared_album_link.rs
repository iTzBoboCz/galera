use base64;
use okapi::openapi3::{
  Object, Responses, SecurityRequirement,
  SecurityScheme, SecuritySchemeData,
};
use rocket::{
  http::Status,
  request::{FromRequest, Request, Outcome},
  Response,
};
use rocket_okapi::{
  gen::OpenApiGenerator,
  request::{OpenApiFromRequest, RequestHeaderInput},
  response::OpenApiResponder,
};
use serde::{Serialize, Deserialize};
use sha2::Digest;
use crate::db::{albums::{select_album, select_album_share_link_by_uuid}};
use crate::DbConn;
use std::str;

#[derive(Debug, Serialize, Deserialize, JsonSchema)]
pub struct SharedAlbumLinkSecurity {
  album_share_link_uuid: String,
  password: Option<String>,
}

/// Encrypts the password.
// TODO: deduplicate later
pub fn hash_password(password: String) -> String {
  let mut hasher = sha2::Sha512::new();
  hasher.update(password);
  // {:x} means format as hexadecimal
  format!("{:X}", hasher.finalize())
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for SharedAlbumLinkSecurity {
  type Error = ();

  /// Implements Request guard for SharedAlbumLinkSecurity.
  async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
    let conn = request.guard::<DbConn>().await.unwrap();

    let headers = request.headers();
    if headers.is_empty() {
      return Outcome::Failure((Status::UnprocessableEntity, ()));
    }
    let authorization_header: Vec<&str> = headers
      .get("authorization")
      .filter(|r| !r.is_empty())
      .collect();

    if authorization_header.is_empty() {
      return Outcome::Failure((Status::UnprocessableEntity, ()));
    }

    let base64_uuid_password_pair: &str = authorization_header[0][5..authorization_header[0].len()].trim();

    let decoded = base64::decode(base64_uuid_password_pair);
    if decoded.is_err() { return Outcome::Failure((Status::UnprocessableEntity, ())) }

    let decoded_unwrap = decoded.unwrap();

    let decoded_str = str::from_utf8(&decoded_unwrap);
    if decoded_str.is_err() { return Outcome::Failure((Status::UnprocessableEntity, ())) }

    let split: Vec<&str> = decoded_str.unwrap().split(':').collect();
    let album_share_link_uuid = split[0].to_string();
    let password_unfiltered = split[1].to_string();
    let hashed_password =  match password_unfiltered.len() {
      0 => None,
      _ => Some(hash_password(password_unfiltered))
    };

    let album_share_link_result = select_album_share_link_by_uuid(&conn, album_share_link_uuid).await;
    if album_share_link_result.is_err() { return Outcome::Failure((Status::InternalServerError, ())) }

    let album_share_link_option = album_share_link_result.unwrap();
    if album_share_link_option.is_none() { return Outcome::Failure((Status::Unauthorized, ())) }

    let album_share_link = album_share_link_option.unwrap();

    // TODO: change select_album() to return Result<Option<Album>>; change status when this happens
    let album = select_album(&conn, album_share_link.album_id).await;
    if album.is_none() { return Outcome::Failure((Status::Unauthorized, ())) }

    let album_share_link_security = SharedAlbumLinkSecurity { album_share_link_uuid: album.unwrap().link, password: hashed_password };

    if album_share_link_security.password != album_share_link.password { return Outcome::Failure((Status::Unauthorized, ())) }

    Outcome::Success(album_share_link_security)
  }
}

impl<'a, 'r> OpenApiFromRequest<'a> for SharedAlbumLinkSecurity {
  fn from_request_input(
    _gen: &mut OpenApiGenerator,
    _name: String,
    _required: bool,
  ) -> rocket_okapi::Result<RequestHeaderInput> {
    let mut security_req = SecurityRequirement::new();
    // each security requirement needs a specific key in the openapi docs
    security_req.insert("BasicSharedAlbumLinkAuth".into(), Vec::new());

    // The scheme for the security needs to be defined as well
    // https://swagger.io/docs/specification/authentication/basic-authentication/
    let security_scheme = SecurityScheme {
      description: Some("requires a base64 encoded string in format `album_share_link_uuid:password` to access".into()),
      // this will show where and under which name the value will be found in the HTTP header
      // in this case, the header key x-api-key will be searched
      // other alternatives are "query", "cookie" according to the openapi specs.
      // [link](https://swagger.io/specification/#security-scheme-object)
      // which also is where you can find examples of how to create a JWT scheme for example
      data: SecuritySchemeData::Http {
        scheme: String::from("basic"),
        bearer_format: None,
      },
      extensions: Object::default(),
    };

    Ok(RequestHeaderInput::Security(
      // scheme identifier is the keyvalue under which this security_scheme will be filed in
      // the openapi.json file
      "BasicSharedAlbumLinkAuth".to_owned(),
      security_scheme,
      security_req,
    ))
  }
}
