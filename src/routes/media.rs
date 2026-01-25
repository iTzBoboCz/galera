use std::sync::Arc;
use crate::auth::shared_album_link::SharedAlbumLinkSecurity;
use crate::auth::token::Claims;
use crate::db;
use crate::db::media::select_media_by_uuid;
use crate::directories::Directories;
use crate::models::{Folder, Media};
use crate::openapi::AUTH_PROTECTED;
use axum::Extension;
use axum::body::Body;
use axum::extract::State;
use axum::http::Request;
use axum::{Json, http::StatusCode};
use axum_extra::routing::TypedPath;
use chrono::NaiveDateTime;
use tracing::error;
use utoipa::ToSchema;
use crate::{AppState, scan};
use serde::{Deserialize, Serialize};
use tokio_util::io::ReaderStream;

// #[derive(JsonSchema)]
#[derive(Serialize, Deserialize, Queryable, ToSchema)]
pub struct MediaResponse {
  pub filename: String,
  pub owner_id: i32,
  pub width: u32,
  pub height: u32,
  pub description: Option<String>,
  pub date_taken: Option<NaiveDateTime>,
  pub uuid: String,
}

impl From<Media> for MediaResponse {
  fn from(media: Media) -> Self {
    MediaResponse { filename: media.filename, owner_id: media.owner_id, width: media.width, height: media.height, description: media.description, date_taken: media.date_taken, uuid: media.uuid }
  }
}

impl From<&Media> for MediaResponse {
  fn from(media: &Media) -> Self {
    MediaResponse { filename: media.filename.clone(), owner_id: media.owner_id, width: media.width, height: media.height, description: media.description.clone(), date_taken: media.date_taken, uuid: media.uuid.clone() }
  }
}

#[derive(TypedPath)]
#[typed_path("/media")]
pub struct MediaRoute;

/// Gets a list of all media
// FIXME: skips new media in /gallery/username/<medianame>; /gallery/username/<some_folder>/<medianame> works
#[utoipa::path(
  get,
  path = "/media",
  security(("BearerAuth" = [])),
  tags = [ "media", "auth:protected" ],
  responses(
    (status = 200, description = "Media tree", body = [MediaResponse]),
    (status = 401, description = "Unauthorized"),
    (status = 403, description = "Forbidden")
  )
)]
pub async fn media_structure(
  _: MediaRoute,
  State(AppState { pool,.. }): State<AppState>,
  Extension(claims): Extension<Arc<Claims>>
) -> Result<Json<Vec<MediaResponse>>, StatusCode> {
  error!("user_id: {}", claims.user_id);

  let structure = db::media::get_media_structure(pool.get().await.unwrap(), claims.user_id).await;

  Ok(Json(structure))
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/media/{media_uuid}")]
pub struct MediaUuidRoute {
  media_uuid: String,
}

/// Returns a media
#[utoipa::path(
  get,
  path = "/media/{media_uuid}",
  tags = ["media", AUTH_PROTECTED],
  security(
    ("BearerAuth" = []),
  ),
  responses(
    (status = 200, description = "Binary media stream", content_type = "application/octet-stream", body = Vec<u8>),
    (status = 401, description = "Unauthorized"),
    (status = 404, description = "Media not found"),
    (status = 500, description = "Internal server error")
  )
)]
pub async fn get_media_by_uuid(
  MediaUuidRoute { media_uuid }: MediaUuidRoute,
  State(AppState { pool,.. }): State<AppState>,
  request: Request<Body>
) -> Result<Body, StatusCode> {
  let media = match select_media_by_uuid(pool.get().await.unwrap(), media_uuid).await {
    Ok(Some(media)) => media,

    Ok(None) => {
      return Err(StatusCode::NOT_FOUND)
    }

    Err(e) => {
      error!("DB error selecting oidc identity: {e}");
      return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }
  };

  if let Some(claims) = request.extensions().get::<Arc<Claims>>() {
    if media.owner_id != claims.user_id {
      return Err(StatusCode::UNAUTHORIZED);
    }

    // TODO: check if non-owner user has permission to access the album (preparation for shared albums)

  } else if let Some(_special) = request.extensions().get::<Arc<SharedAlbumLinkSecurity>>() {
    // TODO: maybe check more things
  } else {
    return Err(StatusCode::UNAUTHORIZED);
  }

  let directories = Directories::new();
  if directories.is_none() { return Err(StatusCode::INTERNAL_SERVER_ERROR); }

  let xdg_data = directories.unwrap().gallery().to_owned();
  if xdg_data.is_none() { return Err(StatusCode::INTERNAL_SERVER_ERROR); }

  let mut folders: Vec<Folder> = vec!();

  let Some(current_folder) = db::folders::select_folder(pool.get().await.unwrap(), media.folder_id).await else { return Err(StatusCode::INTERNAL_SERVER_ERROR); };
  folders.push(current_folder.clone());

  scan::select_parent_folder_recursive(pool, current_folder, media.owner_id, &mut folders);

  let mut path = xdg_data.unwrap();

  if !folders.is_empty() {
    for folder in folders.iter().rev() {
      path = path.join(folder.name.as_str());
    }
  }
  path = path.join(&media.filename);

  // `File` implements `AsyncRead`
  let Ok(file) = tokio::fs::File::open(path).await else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };
  // convert the `AsyncRead` into a `Stream`
  let stream = ReaderStream::new(file);
  // convert the `Stream` into an `axum::body::HttpBody`
  Ok(Body::from_stream(stream))
}

// #[derive(JsonSchema)]
#[derive(Serialize, Deserialize)]
pub struct MediaDescription {
  description: Option<String>
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/media/{media_uuid}/description")]
pub struct MediaUuidDescriptionRoute {
  media_uuid: String,
}

/// Updates description of a media
pub async fn media_update_description(
  MediaUuidDescriptionRoute { media_uuid }: MediaUuidDescriptionRoute,
  State(AppState { pool,.. }): State<AppState>,
  Extension(claims): Extension<Arc<Claims>>,
  Json(description): Json<MediaDescription>
) -> Result<StatusCode, StatusCode> {
  let Some(media_id) = db::media::select_media_id(pool.get().await.unwrap(), media_uuid.clone()).await else {
    return Err(StatusCode::NOT_FOUND);
  };

  let access = db::media::media_user_has_access(pool.get().await.unwrap(), media_uuid, claims.user_id).await;
  if access.is_err() { return Err(StatusCode::INTERNAL_SERVER_ERROR) }

  if !access.unwrap() { return Err(StatusCode::FORBIDDEN) }

  let mut description_option = description.description;

  // check for empty string
  if let Some(string) = description_option {
    description_option = if string.chars().count() > 0 { Some(string) } else { None };
  }

  let result = db::media::update_description(pool.get().await.unwrap(), media_id, description_option).await;

  if result.is_err() { return Err(StatusCode::INTERNAL_SERVER_ERROR) }

  Ok(StatusCode::OK)
}

/// Deletes description of a media
pub async fn media_delete_description(
  MediaUuidDescriptionRoute { media_uuid }: MediaUuidDescriptionRoute,
  State(AppState { pool,.. }): State<AppState>,
  Extension(claims): Extension<Arc<Claims>>
) -> Result<StatusCode, StatusCode> {
  let Some(media_id) = db::media::select_media_id(pool.get().await.unwrap(), media_uuid.clone()).await else {
    return Err(StatusCode::NOT_FOUND);
  };

  let access = db::media::media_user_has_access(pool.get().await.unwrap(), media_uuid, claims.user_id).await;
  if access.is_err() { return Err(StatusCode::INTERNAL_SERVER_ERROR) }

  if !access.unwrap() { return Err(StatusCode::FORBIDDEN) }

  let result = db::media::update_description(pool.get().await.unwrap(), media_id, None).await;

  if result.is_err() { return Err(StatusCode::INTERNAL_SERVER_ERROR) }

  Ok(StatusCode::OK)
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/media/liked")]
pub struct MediaLikedRoute;

/// Returns a list of liked media.
pub async fn get_media_liked_list(
  _: MediaLikedRoute,
  State(AppState { pool,.. }): State<AppState>,
  Extension(claims): Extension<Arc<Claims>>,
) -> Result<Json<Vec<MediaResponse>>, StatusCode> {
  let Ok(liked) = db::media::get_liked_media(pool.get().await.unwrap(), claims.user_id).await else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR)
  };

  let result = liked.iter()
    .map(MediaResponse::from)
    .collect::<Vec<MediaResponse>>();

  Ok(Json(result))
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/media/{media_uuid}/like")]
pub struct MediaUuidLikeRoute {
  media_uuid: String,
}

/// Likes the media.
pub async fn media_like(
  MediaUuidLikeRoute { media_uuid }: MediaUuidLikeRoute,
  State(AppState { pool,.. }): State<AppState>,
  Extension(claims): Extension<Arc<Claims>>,
) -> Result<StatusCode, StatusCode> {
  let Some(media_id) = db::media::select_media_id(pool.get().await.unwrap(), media_uuid).await else {
    return Err(StatusCode::NOT_FOUND);
  };

  // It would be better to return result and have different responses for each error kind.
  // But it looks like that Diesel uses one error kind for multiple different errors and changes only the message.
  let changed_rows = db::media::media_like(pool.get().await.unwrap(), media_id, claims.user_id).await;
  if changed_rows.is_ok() {
    return Ok(StatusCode::OK);
  }

  error!("Inserting like failed: {}", changed_rows.as_ref().unwrap_err());
  Err(StatusCode::CONFLICT)
}

/// Unlikes the media.
pub async fn media_unlike(
  MediaUuidLikeRoute { media_uuid }: MediaUuidLikeRoute,
  State(AppState { pool,.. }): State<AppState>,
  Extension(claims): Extension<Arc<Claims>>,
) -> Result<StatusCode, StatusCode> {
  let Some(media_id) = db::media::select_media_id(pool.get().await.unwrap(), media_uuid).await else {
    return Err(StatusCode::NOT_FOUND);
  };

  let Ok(changed_rows) = db::media::media_unlike(pool.get().await.unwrap(), media_id, claims.user_id).await else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  if changed_rows == 0 {
    return Ok(StatusCode::NO_CONTENT);
  }

  Ok(StatusCode::OK)
}
