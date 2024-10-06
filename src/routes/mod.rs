use std::sync::Arc;
use crate::auth::login::{UserLogin, UserInfo, LoginResponse};
use crate::auth::shared_album_link::{SharedAlbumLinkSecurity, hash_password};
use crate::auth::token::{Claims, ClaimsEncoded};
use crate::db::{self, users::get_user_by_id};
use crate::directories::Directories;
use crate::models::{Album, AlbumShareLink, Folder, Media, NewAlbumMedia, NewAlbumShareLink, NewUser};
use axum::Extension;
use axum::body::Body;
use axum::extract::{State};
use axum::http::Request;
use axum::{Json, http::StatusCode};
use axum_extra::routing::TypedPath;
use tracing::{info, error};
use crate::scan;
use crate::schema::media;
use crate::{ConnectionPool};
use chrono::{NaiveDateTime, Utc};
use diesel::ExpressionMethods;

use diesel::QueryDsl;
use diesel::RunQueryDsl;
use diesel::Table;
// use rocket::{fs::NamedFile, http::Status};
use serde::{Deserialize, Serialize};
// use schemars::JsonSchema;
use tokio_util::io::ReaderStream;

// #[openapi]
// #[get("/")]
// pub async fn index() -> &'static str {
//   "Hello, world!"
// }

#[derive(TypedPath)]
#[typed_path("/user")]
pub struct UserRoute;

/// Creates a new user
pub async fn create_user(
  _: UserRoute,
  State(pool): State<ConnectionPool>,
  Json(user): Json<NewUser>,
) -> Result<StatusCode, StatusCode> {
  if !user.check() { return Err(StatusCode::UNPROCESSABLE_ENTITY) }

  // TODO: investigate passing pool vs connection as parameter
  if !db::users::is_user_unique(pool.get().await.unwrap(), user.clone()).await { return Err(StatusCode::CONFLICT); };

  let new_user = user.hash_password();
  let result = db::users::insert_user(pool.get().await.unwrap(), new_user.clone()).await;
  if result == 0 { return Err(StatusCode::INTERNAL_SERVER_ERROR) }

  info!("A new user was created with name {}", new_user.username);
  Ok(StatusCode::OK)
}

#[derive(TypedPath)]
#[typed_path("/login")]
pub struct LoginRoute;

/// You must provide either a username or an email together with a password.
pub async fn login(
  _: LoginRoute,
  State(pool): State<ConnectionPool>,
  Json(user_login): Json<UserLogin>,
) -> Result<Json<LoginResponse>, StatusCode> {
  let token_option = user_login.hash_password().login(pool.clone()).await;
  if token_option.is_none() { return Err(StatusCode::CONFLICT); }

  let token = token_option.unwrap();

  let user_info = get_user_by_id(pool.get().await.unwrap(), token.user_id).await;
  if user_info.is_none() { return Err(StatusCode::INTERNAL_SERVER_ERROR) }

  let encoded = token.encode();
  if encoded.is_err() { return Err(StatusCode::INTERNAL_SERVER_ERROR) }

  Ok(
    Json(
      LoginResponse::new(
        encoded.unwrap(),
        UserInfo::from(user_info.unwrap())
      )
    )
  )
}

#[derive(TypedPath)]
#[typed_path("/login/refresh")]
pub struct LoginRefreshRoute;

/// Refreshes sent token
// TODO: send token in header instead of body
// https://stackoverflow.com/a/53881397
pub async fn refresh_token(
  _: LoginRefreshRoute,
  State(pool): State<ConnectionPool>,
  Json(encoded_bearer_token): Json<ClaimsEncoded>,
) -> Result<Json<ClaimsEncoded>, StatusCode> {
  let decoded = encoded_bearer_token.clone().decode();
  let bearer_token: Claims;

  // access token is expired - most of the time (token needs to be refreshed because it is expired)
  if decoded.is_err() {
    let expired = match decoded.unwrap_err().kind() {
      jsonwebtoken::errors::ErrorKind::ExpiredSignature => true,
      _ => false
    };

    // the error is not expired token
    if !expired { return Err(StatusCode::UNAUTHORIZED) }

    let temp = encoded_bearer_token.decode_without_validation();

    // couldn't be decoded
    if temp.is_err() { return Err(StatusCode::UNAUTHORIZED) }


    bearer_token = temp.unwrap().claims
  } else {
    // access token is not yet expired
    bearer_token = decoded.unwrap().claims;
  }

  // refresh token is expired
  if bearer_token.is_refresh_token_expired(pool.get().await.unwrap()).await { return Err(StatusCode::UNAUTHORIZED); }

  let new_token = Claims::from_existing(&bearer_token);

  let Some(refresh_token_id) = db::tokens::select_refresh_token_id(pool.get().await.unwrap(), bearer_token.refresh_token()).await else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  Claims::delete_obsolete_access_tokens(pool.get().await.unwrap(), refresh_token_id).await;

  if new_token.add_access_token_to_db(pool, refresh_token_id).await.is_none() { return Err(StatusCode::INTERNAL_SERVER_ERROR); }

  let Ok(new_encoded_token) = new_token.encode() else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  Ok(Json(new_encoded_token))
}

// #[derive(JsonSchema)]
#[derive(Serialize, Deserialize, Queryable)]
pub struct MediaResponse {
  pub filename: String,
  pub owner_id: i32,
  pub width: u32,
  pub height: u32,
  pub description: Option<String>,
  pub date_taken: NaiveDateTime,
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
pub async fn media_structure(
  _: MediaRoute,
  State(pool): State<ConnectionPool>,
  Extension(claims): Extension<Arc<Claims>>
) -> Result<Json<Vec<MediaResponse>>, StatusCode> {
  error!("user_id: {}", claims.user_id);

  let structure = db::media::get_media_structure(pool.get().await.unwrap(), claims.user_id).await;

  Ok(Json(structure))
}

// #[derive(JsonSchema)]
#[derive(Serialize, Deserialize, Queryable)]
pub struct AlbumInsertData {
  pub name: String,
  pub description: Option<String>,
}

// #[derive(JsonSchema)]
#[derive(Serialize, Deserialize, Queryable)]
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
pub async fn create_album(
  _: AlbumRoute,
  State(pool): State<ConnectionPool>,
  Extension(claims): Extension<Arc<Claims>>,
  Json(album_insert_data): Json<AlbumInsertData>
) -> Json<Option<AlbumResponse>> {
  db::albums::insert_album(pool.get().await.unwrap(), claims.user_id, album_insert_data).await;

  let Some(last_insert_id) = db::general::get_last_insert_id(pool.get().await.unwrap()).await else {
    error!("Last insert id was not returned. This may happen if restarting MySQL during scanning.");
    return Json(None);
  };

  let accessible = db::albums::user_has_album_access(pool.get().await.unwrap(), claims.user_id, last_insert_id).await;
  if accessible.is_err() || !accessible.unwrap() { return Json(None); }

  // TODO: impl from u jiné struktury bez ID a hesla
  let Some(album) = db::albums::select_album(pool.get().await.unwrap(), last_insert_id).await else {
    return Json(None);
  };

  Json(Some(AlbumResponse::from(album)))
}

// #[derive(JsonSchema)]
#[derive(Deserialize)]
pub struct AlbumAddMedia {
  album_uuid: String,
  media_uuid: String,
}

#[derive(TypedPath)]
#[typed_path("/album/media")]
pub struct AlbumMediaRoute;

/// Adds media to an album
pub async fn album_add_media(
  _: AlbumMediaRoute,
  State(pool): State<ConnectionPool>,
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
pub async fn get_album_list(
  _: AlbumRoute,
  State(pool): State<ConnectionPool>,
  Extension(claims): Extension<Arc<Claims>>
) -> Json<Vec<AlbumResponse>> {
  let albums = db::albums::get_album_list(pool.get().await.unwrap(), claims.user_id).await;

  let result = albums.iter()
    .map(AlbumResponse::from)
    .collect::<Vec<AlbumResponse>>();

  Json(result)
}

// #[derive(JsonSchema)
#[derive(Serialize, Deserialize, Queryable)]
pub struct AlbumUpdateData {
  pub name: Option<String>,
  pub description: Option<String>,
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/album/:album_uuid/media")]
pub struct AlbumUuidMediaRoute {
  album_uuid: String,
}

/// Gets a list of media in an album
// TODO: consider using the Extension extractor for auth here
pub async fn get_album_structure(
  AlbumUuidMediaRoute { album_uuid }: AlbumUuidMediaRoute,
  State(pool): State<ConnectionPool>,
  request: Request<Body>
) -> Result<Json<Vec<MediaResponse>>, StatusCode> {
  let Some(album_id) = db::albums::select_album_id(pool.get().await.unwrap(), album_uuid).await else {
    return Err(StatusCode::NOT_FOUND);
  };

  let Some(album) = db::albums::select_album(pool.get().await.unwrap(), album_id).await else {
    return Err(StatusCode::NOT_FOUND);
  };

  if let Some(claims) = request.extensions().get::<Arc<Claims>>() {
      if album.owner_id != claims.user_id {
        return Err(StatusCode::UNAUTHORIZED);
      }

      // let accessible = db::albums::user_has_album_access(pool.get().await.unwrap(), claims.user_id, album_id).await;
      // if accessible.is_err() { return Err(StatusCode::InternalServerError) }

      // if !accessible.unwrap() {
      //   return Err(StatusCode::Forbidden);
      // }

      // TODO: check if non-owner user has permission to access the album (preparation for shared albums)
  } else if let Some(special) = request.extensions().get::<Arc<SharedAlbumLinkSecurity>>() {
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

/// Updates already existing album
pub async fn update_album(
  AlbumUuidRoute { album_uuid }: AlbumUuidRoute,
  State(pool): State<ConnectionPool>,
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
pub async fn delete_album(
  AlbumUuidRoute { album_uuid }: AlbumUuidRoute,
  State(pool): State<ConnectionPool>,
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
  if deleted.is_err() { return Err(StatusCode::IM_A_TEAPOT) }

  Ok(StatusCode::OK)
}

// #[derive(JsonSchema)]
#[derive(Serialize, Deserialize)]
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

// #[derive(JsonSchema)]
#[derive(Serialize, Deserialize)]
pub struct SharedAlbumLinkResponse {
  uuid: String,
  expiration: Option<NaiveDateTime>,
}


#[derive(TypedPath, Deserialize)]
#[typed_path("/album/:album_uuid/share/link")]
pub struct AlbumUuidShareLinkRoute {
  album_uuid: String,
}

/// Creates a new album share link.
pub async fn create_album_share_link(
  AlbumUuidShareLinkRoute { album_uuid }: AlbumUuidShareLinkRoute,
  State(pool): State<ConnectionPool>,
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
pub async fn get_album_share_links(
  AlbumUuidShareLinkRoute { album_uuid }: AlbumUuidShareLinkRoute,
  State(pool): State<ConnectionPool>,
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

// #[derive(JsonSchema)]
#[derive(Serialize)]
pub struct AlbumShareLinkBasic {
  pub album_uuid: String,
  pub is_password_protected: bool,
  pub is_expired: bool
}

impl AlbumShareLinkBasic {
  pub fn new(album_share_link: AlbumShareLink, album_uuid: String) -> Self {
    let current_time = NaiveDateTime::from_timestamp(Utc::now().timestamp(), 0);

    Self {
      album_uuid,
      is_expired: album_share_link.expiration.is_some() && album_share_link.expiration.unwrap() < current_time,
      is_password_protected: album_share_link.password.is_some()
     }
  }
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/album/share/link/:album_share_link_link")]
pub struct AlbumShareLinkRoute {
  album_share_link_link: String,
}

/// Gets basic information about album share link from its link.
pub async fn get_album_share_link(
  AlbumShareLinkRoute { album_share_link_link }: AlbumShareLinkRoute,
  State(pool): State<ConnectionPool>
) -> Result<Json<AlbumShareLinkBasic>, StatusCode> {
  let Ok(album_share_link_option) = db::albums::select_album_share_link_by_link(pool.get().await.unwrap(), album_share_link_link).await else {
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

#[derive(TypedPath, Deserialize)]
#[typed_path("/album/share/link/uuid/:album_share_link_uuid")]
pub struct AlbumShareLinkUuidRoute {
  album_share_link_uuid: String,
}

/// Gets basic information about album share link from its uuid.
pub async fn get_album_share_link_uuid(
  AlbumShareLinkUuidRoute { album_share_link_uuid }: AlbumShareLinkUuidRoute,
  State(pool): State<ConnectionPool>
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
pub async fn update_album_share_link(
  AlbumShareLinkUuidRoute { album_share_link_uuid }: AlbumShareLinkUuidRoute,
  State(pool): State<ConnectionPool>,
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
pub async fn delete_album_share_link(
  AlbumShareLinkUuidRoute { album_share_link_uuid }: AlbumShareLinkUuidRoute,
  State(pool): State<ConnectionPool>,
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

#[derive(TypedPath, Deserialize)]
#[typed_path("/scan_media")]
pub struct ScanMediaRoute;

/// Searches for new media
pub async fn scan_media(
  _: ScanMediaRoute,
  State(pool): State<ConnectionPool>,
  Extension(claims): Extension<Arc<Claims>>
) -> Result<StatusCode, StatusCode> {
  let Some(directories) = Directories::new() else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  let Some(xdg_data) = directories.gallery().to_owned() else {
    return Err(StatusCode::INTERNAL_SERVER_ERROR);
  };

  // this thread will run until scanning is complete
  tokio::spawn(scan::scan_root(pool, xdg_data, claims.user_id));

  Ok(StatusCode::OK)
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/album/:album_uuid")]
pub struct AlbumUuidRoute {
  album_uuid: String,
}

#[derive(TypedPath, Deserialize)]
#[typed_path("/media/:media_uuid")]
pub struct MediaUuidRoute {
  media_uuid: String,
}

// /// Returns a media
pub async fn get_media_by_uuid(
  MediaUuidRoute { media_uuid }: MediaUuidRoute,
  State(pool): State<ConnectionPool>,
  request: Request<Body>
) -> Result<Body, StatusCode> {
  let Ok(media) = pool.get().await.unwrap().interact(|c| {
    media::table
      .select(media::table::all_columns())
      .filter(media::dsl::uuid.eq(media_uuid))
      .first::<Media>(c)
      .unwrap()
  }).await else { return Err(StatusCode::NOT_FOUND) };

  if let Some(claims) = request.extensions().get::<Arc<Claims>>() {
    if media.owner_id != claims.user_id {
      return Err(StatusCode::UNAUTHORIZED);
    }

    // TODO: check if non-owner user has permission to access the album (preparation for shared albums)

  } else if let Some(special) = request.extensions().get::<Arc<SharedAlbumLinkSecurity>>() {
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
#[typed_path("/media/:media_uuid/description")]
pub struct MediaUuidDescriptionRoute {
  media_uuid: String,
}

/// Updates description of a media
pub async fn media_update_description(
  MediaUuidDescriptionRoute { media_uuid }: MediaUuidDescriptionRoute,
  State(pool): State<ConnectionPool>,
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
  State(pool): State<ConnectionPool>,
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
  State(pool): State<ConnectionPool>,
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
#[typed_path("/media/:media_uuid/like")]
pub struct MediaUuidLikeRoute {
  media_uuid: String,
}

/// Likes the media.
pub async fn media_like(
  MediaUuidLikeRoute { media_uuid }: MediaUuidLikeRoute,
  State(pool): State<ConnectionPool>,
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
  State(pool): State<ConnectionPool>,
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

// #[derive(JsonSchema)
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SystemInfoPublic {
  operating_system: String,
  server_name: String,
  server_version: String,
  system_architecture: String,
}

impl SystemInfoPublic {
  pub fn new() -> Self {
    Self {
      operating_system: std::env::consts::OS.to_string(),
      server_name: sys_info::hostname().unwrap_or(String::new()),
      server_version: env!("CARGO_PKG_VERSION").to_string(),
      system_architecture: std::env::consts::ARCH.to_string(),
    }
  }
}

#[derive(TypedPath)]
#[typed_path("/system/info/public")]
pub struct SystemInfoPublicRoute;

/// Returns the public system information.
pub async fn system_info_public(
  _: SystemInfoPublicRoute,
) -> Json<SystemInfoPublic> {
  Json(SystemInfoPublic::new())
}
