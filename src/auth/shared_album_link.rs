use axum::{extract::State, middleware::Next, http::{StatusCode, Request}, response::Response, body::Body};
use axum_extra::{TypedHeader, headers::{Authorization, authorization}};
use serde::{Serialize, Deserialize};
use sha2::Digest;
use crate::{AppState, db::albums::{select_album, select_album_share_link_by_uuid}};
use std::{str, sync::Arc};

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
pub async fn shared_album_link(State(AppState { pool,.. }): State<AppState>, TypedHeader(Authorization(special_auth)): TypedHeader<Authorization<authorization::Basic>>, mut req: Request<Body>, next: Next) -> Result<Response, StatusCode> {
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
  req.extensions_mut().insert(Arc::new(album_share_link_security));
  return Ok(next.run(req).await)
}
