use axum::{extract::State, TypedHeader, headers::{Authorization, authorization}, middleware::Next, http::{StatusCode, Request}, response::Response};
use serde::{Serialize, Deserialize};
use sha2::Digest;
use crate::{db::{albums::{select_album, select_album_share_link_by_uuid}}, ConnectionPool};
use std::str;

// #[derive(JsonSchema)]
#[derive(Debug, Serialize, Deserialize)]
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

/// Implements Request guard for SharedAlbumLinkSecurity.
pub async fn shared_album_link<B>(State(pool): State<ConnectionPool>, TypedHeader(Authorization(special_auth)): TypedHeader<Authorization<authorization::Basic>>, mut req: Request<B>, next: Next<B>) -> Result<Response, StatusCode> {
  let album_share_link_uuid = special_auth.username().to_string();
  let password = special_auth.password().to_string();
  let hashed_password =  match password.len() {
    0 => None,
    _ => Some(hash_password(password))
  };

  let Ok(album_share_link_option) = select_album_share_link_by_uuid(pool.get().await.unwrap(), album_share_link_uuid).await else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  let Some(album_share_link) = album_share_link_option else {
    return Err(StatusCode::UNAUTHORIZED);
  };

  // // TODO: change select_album() to return Result<Option<Album>>; change status when this happens
  let Some(album) = select_album(pool.get().await.unwrap(), album_share_link.album_id).await else {
    return Err(StatusCode::UNAUTHORIZED);
  };

  let album_share_link_security = SharedAlbumLinkSecurity { album_share_link_uuid: album.link, password: hashed_password };

  if album_share_link_security.password != album_share_link.password { return Err(StatusCode::UNAUTHORIZED) }

  // insert the current user into a request extension so the handler can
  // extract it
  req.extensions_mut().insert(album_share_link_security);
  return Ok(next.run(req).await)
}

// impl<'a, 'r> OpenApiFromRequest<'a> for SharedAlbumLinkSecurity {
//   fn from_request_input(
//     _gen: &mut OpenApiGenerator,
//     _name: String,
//     _required: bool,
//   ) -> rocket_okapi::Result<RequestHeaderInput> {
//     let mut security_req = SecurityRequirement::new();
//     // each security requirement needs a specific key in the openapi docs
//     security_req.insert("BasicSharedAlbumLinkAuth".into(), Vec::new());

//     // The scheme for the security needs to be defined as well
//     // https://swagger.io/docs/specification/authentication/basic-authentication/
//     let security_scheme = SecurityScheme {
//       description: Some("requires a base64 encoded string in format `album_share_link_uuid:password` to access".into()),
//       // this will show where and under which name the value will be found in the HTTP header
//       // in this case, the header key x-api-key will be searched
//       // other alternatives are "query", "cookie" according to the openapi specs.
//       // [link](https://swagger.io/specification/#security-scheme-object)
//       // which also is where you can find examples of how to create a JWT scheme for example
//       data: SecuritySchemeData::Http {
//         scheme: String::from("basic"),
//         bearer_format: None,
//       },
//       extensions: Object::default(),
//     };

//     Ok(RequestHeaderInput::Security(
//       // scheme identifier is the keyvalue under which this security_scheme will be filed in
//       // the openapi.json file
//       "BasicSharedAlbumLinkAuth".to_owned(),
//       security_scheme,
//       security_req,
//     ))
//   }
// }
