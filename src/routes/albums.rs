use std::sync::Arc;
use crate::auth::mixed_auth::MixedAuth;
use crate::auth::shared_album_link::hash_password;
use crate::auth::token::Claims;
use crate::models::{Album, AlbumShareLink, NewAlbumMedia, NewAlbumShareLink};
use crate::openapi::tags::{ALBUMS, AUTH_MIXED, AUTH_PROTECTED};
use crate::routes::media::MediaResponse;
use axum::Extension;
use axum::extract::State;
use axum::{Json, http::StatusCode};
use axum_extra::routing::TypedPath;
use tracing::error;
use utoipa::ToSchema;
use crate::{AppState, db};
use chrono::{NaiveDateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Queryable, ToSchema)]
pub struct AlbumInsertData {
  pub name: String,
  pub description: Option<String>,
}

#[derive(Serialize, Deserialize, Queryable, ToSchema)]
pub struct AlbumResponse {
  pub owner_id: i32,
  pub name: String,
  pub description: Option<String>,
  pub created_at: NaiveDateTime,
  pub thumbnail_link: Option<String>,
  pub link: String
}

impl From<Album> for AlbumResponse {
  fn from(album: Album) -> Self {
    AlbumResponse { owner_id: album.owner_id, name: album.name, description: album.description, created_at: album.created_at, thumbnail_link: album.thumbnail_link, link: album.link }
  }
}

impl From<&Album> for AlbumResponse {
  fn from(album: &Album) -> Self {
    AlbumResponse { owner_id: album.owner_id, name: album.name.clone(), description: album.description.clone(), created_at: album.created_at, thumbnail_link: album.thumbnail_link.clone(), link: album.link.clone() }
  }
}

// impl From<NewAlbum> for AlbumResponse {
//   fn from(album: NewAlbum) -> Self {
//     AlbumResponse { owner_id: album.owner_id, name: album.name, description: album.description, created_at: album.created_at, thumbnail_link: None, link: album.link }
//   }
// }

#[derive(TypedPath)]
#[typed_path("/album")]
pub struct AlbumRoute;

/// Creates a new album
// TODO: change response later
#[utoipa::path(
  post,
  path = "/album",
  security(("BearerAuth" = [])),
  tags = [ALBUMS, AUTH_PROTECTED],
  request_body = AlbumInsertData,
  responses(
    (status = 200, description = "Album created (or null on failure)", body = Option<AlbumResponse>),
    (status = 401, description = "Unauthorized"),
    (status = 500, description = "Internal server error")
  )
)]
pub async fn create_album(
  _: AlbumRoute,
  State(AppState { pool,.. }): State<AppState>,
  Extension(claims): Extension<Arc<Claims>>,
  Json(album_insert_data): Json<AlbumInsertData>
) -> Json<Option<AlbumResponse>> {
  let Ok(album_id) = db::albums::insert_album(pool.get().await.unwrap(), claims.user_id, album_insert_data).await else {
    error!("Problem when inserting an album.");
    return Json(None);
  };

  let accessible = db::albums::user_has_album_access(pool.get().await.unwrap(), claims.user_id, album_id).await;
  if accessible.is_err() || !accessible.unwrap() { return Json(None); }

  // TODO: impl from u jiné struktury bez ID a hesla
  let Some(album) = db::albums::select_album(pool.get().await.unwrap(), album_id).await else {
    return Json(None);
  };

  Json(Some(AlbumResponse::from(album)))
}

#[derive(Deserialize, ToSchema)]
pub struct AlbumAddMedia {
  album_uuid: String,
  media_uuid: String,
}

#[derive(TypedPath)]
#[typed_path("/album/media")]
pub struct AlbumMediaRoute;

/// Adds media to an album
#[utoipa::path(
  post,
  path = "/album/media",
  security(("BearerAuth" = [])),
  tags = [ALBUMS, AUTH_PROTECTED],
  request_body = Vec<AlbumAddMedia>,
  responses(
    (status = 200, description = "Media added to album"),
    (status = 400, description = "Bad request"),
    (status = 401, description = "Unauthorized"),
    (status = 403, description = "Forbidden"),
    (status = 500, description = "Internal server error")
  )
)]
pub async fn album_add_media(
  _: AlbumMediaRoute,
  State(AppState { pool,.. }): State<AppState>,
  Extension(claims): Extension<Arc<Claims>>,
  Json(list_of_media): Json<Vec<AlbumAddMedia>>
) -> Result<(), StatusCode> {
  let mut transformed = vec![];

  // TODO: optimise this so it doesn't check the same data multiple times
  for new in list_of_media {
    let album_id = db::albums::select_album_id(pool.get().await.unwrap(), new.album_uuid).await;
    if album_id.is_none() { continue; }

    let album_access = db::albums::user_has_album_access(pool.get().await.unwrap(), claims.user_id, album_id.unwrap()).await;
    if album_access.is_err() { return Err(StatusCode::INTERNAL_SERVER_ERROR) };
    if !album_access.unwrap() { return Err(StatusCode::FORBIDDEN) }

    let media_access = db::media::media_user_has_access(pool.get().await.unwrap(), new.media_uuid.clone(), claims.user_id).await;
    if media_access.is_err() { return Err(StatusCode::INTERNAL_SERVER_ERROR) };
    if !media_access.unwrap() { return Err(StatusCode::FORBIDDEN) }

    let media_id = db::media::select_media_id(pool.get().await.unwrap(), new.media_uuid).await;
    if media_id.is_none() { continue; }

    // skip media that is already present in the album
    let has_media = db::albums::album_already_has_media(pool.get().await.unwrap(), album_id.unwrap(), media_id.unwrap()).await;
    if has_media.is_err() { return Err(StatusCode::INTERNAL_SERVER_ERROR) };

    if has_media.unwrap() { continue; }

    transformed.push(NewAlbumMedia {
      album_id: album_id.unwrap(),
      media_id: media_id.unwrap()
    })
  }

  let r = db::albums::album_add_media(pool.get().await.unwrap(), transformed).await;
  if r.is_none() {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  }

  Ok(())
}

/// Retrieves a list of albums of an authenticated user
#[utoipa::path(
  get,
  path = "/album",
  security(("BearerAuth" = [])),
  tags = [ALBUMS, AUTH_PROTECTED],
  responses(
    (status = 200, description = "Album list", body = Vec<AlbumResponse>),
    (status = 401, description = "Unauthorized")
  )
)]
pub async fn get_album_list(
  _: AlbumRoute,
  State(AppState { pool,.. }): State<AppState>,
  Extension(claims): Extension<Arc<Claims>>
) -> Json<Vec<AlbumResponse>> {
  let albums = db::albums::get_album_list(pool.get().await.unwrap(), claims.user_id).await;

  let result = albums.iter()
    .map(AlbumResponse::from)
    .collect::<Vec<AlbumResponse>>();

  Json(result)
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/album/{album_uuid}/media")]
pub struct AlbumUuidMediaRoute {
  album_uuid: String,
}

/// Gets a list of media in an album
// TODO: consider using the Extension extractor for auth here
#[utoipa::path(
  get,
  path = "/album/{album_uuid}/media",
  params(
    ("album_uuid" = String, Path, description = "Album UUID")
  ),
  security(
    ("BearerAuth" = []),
    ("BasicSharedAlbumLinkAuth" = [])
  ),
  tags = [ALBUMS, AUTH_MIXED],
  responses(
    (status = 200, description = "Album media", body = Vec<MediaResponse>),
    (status = 401, description = "Unauthorized"),
    (status = 404, description = "Album not found"),
    (status = 500, description = "Internal server error")
  )
)]
pub async fn get_album_structure(
  AlbumUuidMediaRoute { album_uuid }: AlbumUuidMediaRoute,
  State(AppState { pool,.. }): State<AppState>,
  Extension(auth): Extension<Arc<MixedAuth>>,
) -> Result<Json<Vec<MediaResponse>>, StatusCode> {
  let Some(album_id) = db::albums::select_album_id(pool.get().await.unwrap(), album_uuid).await else {
    return Err(StatusCode::NOT_FOUND);
  };

  let Some(album) = db::albums::select_album(pool.get().await.unwrap(), album_id).await else {
    return Err(StatusCode::NOT_FOUND);
  };

  if let Some(claims) = auth.claims.as_deref() {
      if album.owner_id != claims.user_id {
        return Err(StatusCode::UNAUTHORIZED);
      }

      // let accessible = db::albums::user_has_album_access(pool.get().await.unwrap(), claims.user_id, album_id).await;
      // if accessible.is_err() { return Err(StatusCode::InternalServerError) }

      // if !accessible.unwrap() {
      //   return Err(StatusCode::Forbidden);
      // }

      // TODO: check if non-owner user has permission to access the album (preparation for shared albums)
  } else if let Some(_special) = auth.shared_album_link.as_deref() {
      // check if the shared album link has access to the album
    // TODO: maybe check more things
  } else {
    return Err(StatusCode::UNAUTHORIZED);
  };

  let Ok(structure) = db::albums::get_album_media(pool.get().await.unwrap(), album.id).await else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  let result = structure.iter()
    .map(MediaResponse::from)
    .collect::<Vec<MediaResponse>>();

  Ok(Json(result))
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/album/{album_uuid}")]
pub struct AlbumUuidRoute {
  album_uuid: String,
}

// #[derive(JsonSchema)
#[derive(Serialize, Deserialize, Queryable, ToSchema)]
pub struct AlbumUpdateData {
  pub name: Option<String>,
  pub description: Option<String>,
}

/// Updates already existing album
#[utoipa::path(
  put,
  path = "/album/{album_uuid}",
  params(
    ("album_uuid" = String, Path, description = "Album UUID")
  ),
  request_body = AlbumUpdateData,
  security(("BearerAuth" = [])),
  tags = [ALBUMS, AUTH_PROTECTED],
  responses(
    (status = 200, description = "Album updated"),
    (status = 204, description = "No changes"),
    (status = 401, description = "Unauthorized"),
    (status = 403, description = "Forbidden"),
    (status = 404, description = "Album not found"),
    (status = 422, description = "Unprocessable entity"),
    (status = 500, description = "Internal server error")
  )
)]
pub async fn update_album(
  AlbumUuidRoute { album_uuid }: AlbumUuidRoute,
  State(AppState { pool,.. }): State<AppState>,
  Extension(claims): Extension<Arc<Claims>>,
  Json(album_update_data): Json<AlbumUpdateData>
) -> Result<StatusCode, StatusCode> {
  if album_update_data.name.is_none() && album_update_data.description.is_none() {
    return Err(StatusCode::UNPROCESSABLE_ENTITY);
  }

  let album_id_option = db::albums::select_album_id(pool.get().await.unwrap(), album_uuid).await;
  if album_id_option.is_none() {
    return Err(StatusCode::NOT_FOUND);
  }

  let album_id = album_id_option.unwrap();

  let accessible = db::albums::user_has_album_access(pool.get().await.unwrap(), claims.user_id, album_id).await;
  if accessible.is_err() { return Err(StatusCode::INTERNAL_SERVER_ERROR) }

  if !accessible.unwrap() {
    return Err(StatusCode::FORBIDDEN);
  }

  let changed_rows = db::albums::update_album(pool.get().await.unwrap(), album_id, album_update_data).await;
  error!("changed: {:?}", changed_rows);
  if changed_rows.is_none() { return Err(StatusCode::INTERNAL_SERVER_ERROR) }

  if changed_rows.unwrap() == 0 {
    return Ok(StatusCode::NO_CONTENT);
  }

  Ok(StatusCode::OK)
}

/// Deletes an album
#[utoipa::path(
  delete,
  path = "/album/{album_uuid}",
  params(
    ("album_uuid" = String, Path, description = "Album UUID")
  ),
  security(("BearerAuth" = [])),
  tags = [ALBUMS, AUTH_PROTECTED],
  responses(
    (status = 200, description = "Album deleted"),
    (status = 401, description = "Unauthorized"),
    (status = 403, description = "Forbidden"),
    (status = 404, description = "Album not found"),
    (status = 500, description = "Internal server error")
  )
)]
pub async fn delete_album(
  AlbumUuidRoute { album_uuid }: AlbumUuidRoute,
  State(AppState { pool,.. }): State<AppState>,
  Extension(claims): Extension<Arc<Claims>>
) -> Result<StatusCode, StatusCode> {
  let album_id_option = db::albums::select_album_id(pool.get().await.unwrap(), album_uuid).await;
  if album_id_option.is_none() {
    return Err(StatusCode::NOT_FOUND);
  }

  let album_id = album_id_option.unwrap();

  let album = db::albums::select_album(pool.get().await.unwrap(), album_id).await;

  if album.is_none() { return Err(StatusCode::NOT_FOUND); }

  let accessible = db::albums::user_has_album_access(pool.get().await.unwrap(), claims.user_id, album_id).await;
  if accessible.is_err() { return Err(StatusCode::INTERNAL_SERVER_ERROR) }

  if !accessible.unwrap() {
    return Err(StatusCode::FORBIDDEN);
  }

  let deleted = db::albums::delete_album(pool.get().await.unwrap(), album_id).await;
  if deleted.is_err() { return Err(StatusCode::INTERNAL_SERVER_ERROR) }

  Ok(StatusCode::OK)
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct AlbumShareLinkInsert {
  pub expiration: Option<NaiveDateTime>,
  pub password: Option<String>,
}

impl AlbumShareLinkInsert {
  // Normalizes passwords and hashes them if they are not None
  pub fn normalize_and_hash_password(self) -> Self {
    if self.password.is_none() { return self }

    let password = self.password.unwrap();

    let hashed_password = match password.len() {
      0 => None,
      _ => Some(hash_password(password))
    };

    Self {
      expiration: self.expiration,
      password: hashed_password,
    }
  }
}

#[derive(Serialize, Deserialize, ToSchema)]
pub struct SharedAlbumLinkResponse {
  uuid: String,
  expiration: Option<NaiveDateTime>,
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/album/{album_uuid}/share/link")]
pub struct AlbumUuidShareLinkRoute {
  album_uuid: String,
}

/// Creates a new album share link.
#[utoipa::path(
  post,
  path = "/album/{album_uuid}/share/link",
  params(
    ("album_uuid" = String, Path, description = "Album UUID")
  ),
  request_body = AlbumShareLinkInsert,
  security(("BearerAuth" = [])),
  tags = [ALBUMS, AUTH_PROTECTED],
  responses(
    (status = 200, description = "Share link created", body = SharedAlbumLinkResponse),
    (status = 401, description = "Unauthorized"),
    (status = 403, description = "Forbidden"),
    (status = 404, description = "Album not found"),
    (status = 500, description = "Internal server error")
  )
)]
pub async fn create_album_share_link(
  AlbumUuidShareLinkRoute { album_uuid }: AlbumUuidShareLinkRoute,
  State(AppState { pool,.. }): State<AppState>,
  Extension(claims): Extension<Arc<Claims>>,
  album_share_link_insert: Option<Json<AlbumShareLinkInsert>>
) -> Result<Json<SharedAlbumLinkResponse>, StatusCode> {
  let Some(album_id) = db::albums::select_album_id(pool.get().await.unwrap(), album_uuid).await else {
    return Err(StatusCode::NOT_FOUND);
  };

  let Some(album) = db::albums::select_album(pool.get().await.unwrap(), album_id).await else {
    return Err(StatusCode::NOT_FOUND);
  };

  if album.owner_id != claims.user_id { return Err(StatusCode::FORBIDDEN) }

  let mut album_share_link_insert_inner = match album_share_link_insert {
    Some(Json(album_share_link)) => album_share_link,
    None => AlbumShareLinkInsert {
      expiration: None,
      password: None
    }
  };

  album_share_link_insert_inner = album_share_link_insert_inner.normalize_and_hash_password();

  let album_share_link = NewAlbumShareLink::new(album_id, album_share_link_insert_inner.password, album_share_link_insert_inner.expiration);

  // It would be better to return result and have different responses for each error kind.
  // But it looks like that Diesel uses one error kind for multiple different errors and changes only the message.
  let changed_rows = db::albums::insert_album_share_link(pool.get().await.unwrap(), album_share_link.clone()).await;
  if changed_rows.is_err() { return Err(StatusCode::INTERNAL_SERVER_ERROR) }
  if changed_rows.unwrap() == 0 { return Err(StatusCode::INTERNAL_SERVER_ERROR) }

  Ok(
    Json(
      SharedAlbumLinkResponse {
        uuid: album_share_link.uuid,
        expiration: album_share_link.expiration
      }
    )
  )
}

impl From<&AlbumShareLink> for SharedAlbumLinkResponse {
  fn from(album_share_link: &AlbumShareLink) -> Self {
    Self { uuid: album_share_link.uuid.clone(), expiration: album_share_link.expiration }
  }
}

/// Gets a list of album share links.
#[utoipa::path(
  get,
  path = "/album/{album_uuid}/share/link",
  params(
    ("album_uuid" = String, Path, description = "Album UUID")
  ),
  security(("BearerAuth" = [])),
  tags = [ALBUMS, AUTH_PROTECTED],
  responses(
    (status = 200, description = "Album share links", body = Vec<SharedAlbumLinkResponse>),
    (status = 401, description = "Unauthorized"),
    (status = 403, description = "Forbidden"),
    (status = 404, description = "Album not found"),
    (status = 500, description = "Internal server error")
  )
)]
pub async fn get_album_share_links(
  AlbumUuidShareLinkRoute { album_uuid }: AlbumUuidShareLinkRoute,
  State(AppState { pool,.. }): State<AppState>,
  Extension(claims): Extension<Arc<Claims>>
) -> Result<Json<Vec<SharedAlbumLinkResponse>>, StatusCode> {
  let Some(album_id) = db::albums::select_album_id(pool.get().await.unwrap(), album_uuid).await else {
    return Err(StatusCode::NOT_FOUND);
  };

  let Some(album) = db::albums::select_album(pool.get().await.unwrap(), album_id).await else {
    return Err(StatusCode::NOT_FOUND);
  };

  if album.owner_id != claims.user_id { return Err(StatusCode::FORBIDDEN) }

  let Ok(links) = db::albums::select_album_share_links(pool.get().await.unwrap(), album_id).await else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  let result = links.iter()
    .map(SharedAlbumLinkResponse::from)
    .collect::<Vec<SharedAlbumLinkResponse>>();

  Ok(Json(result))
}

#[derive(Serialize, ToSchema)]
pub struct AlbumShareLinkBasic {
  pub album_uuid: String,
  pub is_password_protected: bool,
  pub is_expired: bool
}

impl AlbumShareLinkBasic {
  pub fn new(album_share_link: AlbumShareLink, album_uuid: String) -> Self {
    let current_time = Utc::now().naive_utc();

    Self {
      album_uuid,
      is_expired: album_share_link.expiration.is_some_and(|exp| exp < current_time),
      is_password_protected: album_share_link.password.is_some()
     }
  }
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/album/share/link/{album_share_link_uuid}")]
pub struct AlbumShareLinkUuidRoute {
  album_share_link_uuid: String,
}

/// Gets basic information about album share link.
#[utoipa::path(
  get,
  path = "/album/share/link/{album_share_link_uuid}",
  params(
    ("album_share_link_uuid" = String, Path, description = "Album Share Link UUID")
  ),
  security(("BearerAuth" = [])),
  tags = [ALBUMS, AUTH_PROTECTED],
  responses(
    (status = 200, description = "Share link info", body = AlbumShareLinkBasic),
    (status = 404, description = "Not found"),
    (status = 500, description = "Internal server error")
  )
)]
pub async fn get_album_share_link(
  AlbumShareLinkUuidRoute { album_share_link_uuid }: AlbumShareLinkUuidRoute,
  State(AppState { pool,.. }): State<AppState>
) -> Result<Json<AlbumShareLinkBasic>, StatusCode> {
  let Ok(album_share_link_option) = db::albums::select_album_share_link_by_uuid(pool.get().await.unwrap(), album_share_link_uuid).await else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  let Some(album_share_link) = album_share_link_option else {
    return Err(StatusCode::NOT_FOUND);
  };

  let album = db::albums::select_album(pool.get().await.unwrap(), album_share_link.album_id).await;
  if album.is_none() { return Err(StatusCode::INTERNAL_SERVER_ERROR)  }

  Ok(
    Json(
      AlbumShareLinkBasic::new(album_share_link, album.unwrap().link)
    )
  )
}

/// Updates already existing album share link.
#[utoipa::path(
  put,
  path = "/album/share/link/{album_share_link_uuid}",
  request_body = AlbumShareLinkInsert,
  params(
    ("album_share_link_uuid" = String, Path, description = "Album Share Link UUID")
  ),
  security(("BearerAuth" = [])),
  tags = [ALBUMS, AUTH_PROTECTED],
  responses(
    (status = 200, description = "Album share link updated"),
    (status = 204, description = "No changes"),
    (status = 401, description = "Unauthorized"),
    (status = 403, description = "Forbidden"),
    (status = 404, description = "Not found"),
    (status = 500, description = "Internal server error")
  )
)]
pub async fn update_album_share_link(
  AlbumShareLinkUuidRoute { album_share_link_uuid }: AlbumShareLinkUuidRoute,
  State(AppState { pool,.. }): State<AppState>,
  Extension(claims): Extension<Arc<Claims>>,
  Json(album_share_link_insert): Json<AlbumShareLinkInsert>
) -> Result<StatusCode, StatusCode> {
  let Ok(album_share_link_option) = db::albums::select_album_share_link_by_uuid(pool.get().await.unwrap(), album_share_link_uuid).await else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  let Some(album_share_link) = album_share_link_option else {
    return Err(StatusCode::NOT_FOUND)
  };

  let Some(album) = db::albums::select_album(pool.get().await.unwrap(), album_share_link.album_id).await else {
    return Err(StatusCode::NOT_FOUND);
  };

  if album.owner_id != claims.user_id { return Err(StatusCode::FORBIDDEN) }

  let Ok(changed_rows) = db::albums::update_album_share_link(pool.get().await.unwrap(), album_share_link.id, album_share_link_insert.normalize_and_hash_password()).await else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR)
  };

  if changed_rows == 0 {
    return Ok(StatusCode::NO_CONTENT);
  }

  Ok(StatusCode::OK)
}

/// Deletes an album share link.
#[utoipa::path(
  delete,
  path = "/album/share/link/{album_share_link_uuid}",
  params(
    ("album_share_link_uuid" = String, Path, description = "Album Share Link UUID")
  ),
  security(("BearerAuth" = [])),
  tags = [ALBUMS, AUTH_PROTECTED],
  responses(
    (status = 200, description = "Album share link deleted"),
    (status = 204, description = "Nothing deleted"),
    (status = 401, description = "Unauthorized"),
    (status = 403, description = "Forbidden"),
    (status = 404, description = "Not found"),
    (status = 500, description = "Internal server error")
  )
)]
pub async fn delete_album_share_link(
  AlbumShareLinkUuidRoute { album_share_link_uuid }: AlbumShareLinkUuidRoute,
  State(AppState { pool,.. }): State<AppState>,
  Extension(claims): Extension<Arc<Claims>>
) -> Result<StatusCode, StatusCode> {
  let Ok(album_share_link_option) = db::albums::select_album_share_link_by_uuid(pool.get().await.unwrap(), album_share_link_uuid.clone()).await else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  let Some(album_share_link) = album_share_link_option else {
    return Err(StatusCode::NOT_FOUND);
  };

  let Some(album) = db::albums::select_album(pool.get().await.unwrap(), album_share_link.album_id).await else {
    return Err(StatusCode::NOT_FOUND);
  };

  if album.owner_id != claims.user_id { return Err(StatusCode::FORBIDDEN) }

  let Ok(deleted) = db::albums::delete_album_share_link(pool.get().await.unwrap(), album_share_link_uuid).await else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  if deleted == 0 {
    return Ok(StatusCode::NO_CONTENT);
  }

  Ok(StatusCode::OK)
}
