use crate::auth::login::{UserLogin, UserInfo, LoginResponse};
use crate::auth::token::{Claims, ClaimsEncoded};
use crate::db::{self, users::get_user_by_id};
use crate::models::{Album, AlbumShareLink, Folder, Media, NewAlbum, NewAlbumMedia, NewAlbumShareLink, NewUser};
use crate::scan;
use crate::schema::media;
use crate::DbConn;
use chrono::{NaiveDateTime, Utc};
use diesel::ExpressionMethods;
use diesel::OptionalExtension;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use diesel::Table;
use rocket::{fs::NamedFile, http::Status};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use rocket::serde::json::Json;

#[openapi]
#[get("/")]
pub async fn index() -> &'static str {
  "Hello, world!"
}

/// Creates a new user
#[openapi]
#[post("/user", data = "<user>", format = "json")]
pub async fn create_user(conn: DbConn, user: Json<NewUser>) -> Result<Status, Status> {
  if !user.check() { return Err(Status::UnprocessableEntity) }

  if !db::users::is_user_unique(&conn, user.0.clone()).await { return Err(Status::Conflict); };

  let new_user = user.into_inner().hash_password();
  let result = db::users::insert_user(&conn, new_user.clone()).await;
  if result == 0 { return Err(Status::InternalServerError) }

  info!("A new user was created with name {}", new_user.username);
  Ok(Status::Ok)
}

/// You must provide either a username or an email together with a password.
#[openapi]
#[post("/login", data = "<user_login>", format = "json")]
pub async fn login(conn: DbConn, user_login: Json<UserLogin>) -> Result<Json<LoginResponse>, Status> {
  let token_option = user_login.into_inner().hash_password().login(&conn).await;
  if token_option.is_none() { return Err(Status::Conflict); }

  let token = token_option.unwrap();

  let user_info = get_user_by_id(&conn, token.user_id).await;
  if user_info.is_none() { return Err(Status::InternalServerError) }

  let encoded = token.encode();
  if encoded.is_err() { return Err(Status::InternalServerError) }

  Ok(
    Json(
      LoginResponse::new(
        encoded.unwrap(),
        UserInfo::from(user_info.unwrap())
      )
    )
  )
}

/// Refreshes sent token
// TODO: send token in header instead of body
// https://stackoverflow.com/a/53881397
#[openapi]
#[post("/login/refresh", data = "<encoded_bearer_token>", format = "json")]
pub async fn refresh_token(conn: DbConn, encoded_bearer_token: Json<ClaimsEncoded>) -> Result<Json<ClaimsEncoded>, Status> {
  let bearer_token_result = encoded_bearer_token.into_inner();
  let decoded = bearer_token_result.clone().decode();
  let bearer_token: Claims;

  // access token is expired - most of the time (token needs to be refreshed because it is expired)
  if decoded.is_err() {
    let expired = match decoded.unwrap_err().kind() {
      jsonwebtoken::errors::ErrorKind::ExpiredSignature => true,
      _ => false
    };

    // the error is not expired token
    if !expired { return Err(Status::Unauthorized) }

    let temp = bearer_token_result.decode_without_validation();

    // couldn't be decoded
    if temp.is_err() { return Err(Status::Unauthorized) }


    bearer_token = temp.unwrap().claims
  } else {
    // access token is not yet expired
    bearer_token = decoded.unwrap().claims;
  }

  // refresh token is expired
  if bearer_token.is_refresh_token_expired(&conn).await { return Err(Status::Unauthorized); }

  let new_token = Claims::from_existing(&bearer_token);

  let refresh_token_id = db::tokens::select_refresh_token_id(&conn, bearer_token.refresh_token()).await;
  if refresh_token_id.is_none() { return Err(Status::InternalServerError); }

  Claims::delete_obsolete_access_tokens(&conn, refresh_token_id.unwrap()).await;

  if new_token.add_access_token_to_db(&conn, refresh_token_id.unwrap()).await.is_none() { return Err(Status::InternalServerError); }

  let new_encoded_token = new_token.encode();
  if new_encoded_token.is_err() { return Err(Status::InternalServerError); }

  Ok(Json(new_encoded_token.unwrap()))
}

#[derive(Serialize, Deserialize, JsonSchema, Queryable)]
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

/// Gets a list of all media
// FIXME: skips new media in /gallery/username/<medianame>; /gallery/username/<some_folder>/<medianame> works
#[openapi]
#[get("/media")]
pub async fn media_structure(claims: Claims, conn: DbConn) -> Json<Vec<MediaResponse>> {
  error!("user_id: {}", claims.user_id);

  let structure = db::media::get_media_structure(&conn, claims.user_id).await;

  Json(structure)
}

#[derive(Serialize, Deserialize, JsonSchema, Queryable)]
pub struct AlbumInsertData {
  pub name: String,
  pub description: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema, Queryable)]
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

impl From<NewAlbum> for AlbumResponse {
  fn from(album: NewAlbum) -> Self {
    AlbumResponse { owner_id: album.owner_id, name: album.name, description: album.description, created_at: album.created_at, thumbnail_link: None, link: album.link }
  }
}

/// Creates a new album
#[openapi]
#[post("/album", data = "<album_insert_data>", format = "json")]
pub async fn create_album(claims: Claims, conn: DbConn, album_insert_data: Json<AlbumInsertData>) -> Json<Option<AlbumResponse>> {
  db::albums::insert_album(&conn, claims.user_id, album_insert_data.into_inner()).await;

  let last_insert_id = db::general::get_last_insert_id(&conn).await;

  if last_insert_id.is_none() {
    error!("Last insert id was not returned. This may happen if restarting MySQL during scanning.");
    return Json(None);
  }

  let accessible = db::albums::user_has_album_access(&conn, claims.user_id, last_insert_id.unwrap()).await;
  if accessible.is_err() || !accessible.unwrap() { return Json(None); }

  // TODO: impl from u jiné struktury bez ID a hesla
  let album = db::albums::select_album(&conn, last_insert_id.unwrap()).await;
  if album.is_none() { return Json(None); }

  Json(Some(AlbumResponse::from(album.unwrap())))
}

#[derive(Deserialize, JsonSchema)]
pub struct AlbumAddMedia {
  album_uuid: String,
  media_uuid: String,
}

/// Adds media to an album
#[openapi]
#[post("/album/media", data = "<list_of_media>", format = "json")]
pub async fn album_add_media(claims: Claims, conn: DbConn, list_of_media: Json<Vec<AlbumAddMedia>>) -> Result<(), Status> {
  let mut transformed = vec![];

  // TODO: optimise this so it doesn't check the same data multiple times
  for new in list_of_media.into_inner() {
    let album_id = db::albums::select_album_id(&conn, new.album_uuid).await;
    if album_id.is_none() { continue; }

    let album_access = db::albums::user_has_album_access(&conn, claims.user_id, album_id.unwrap()).await;
    if album_access.is_err() { return Err(Status::InternalServerError) };
    if !album_access.unwrap() { return Err(Status::Forbidden) }

    let media_access = db::media::media_user_has_access(&conn, new.media_uuid.clone(), claims.user_id).await;
    if media_access.is_err() { return Err(Status::InternalServerError) };
    if !media_access.unwrap() { return Err(Status::Forbidden) }

    let media_id = db::media::select_media_id(&conn, new.media_uuid).await;
    if media_id.is_none() { continue; }

    // skip media that is already present in the album
    let has_media = db::albums::album_already_has_media(&conn, album_id.unwrap(), media_id.unwrap()).await;
    if has_media.is_err() { return Err(Status::InternalServerError) };

    if has_media.unwrap() { continue; }

    transformed.push(NewAlbumMedia {
      album_id: album_id.unwrap(),
      media_id: media_id.unwrap()
    })
  }

  let r = db::albums::album_add_media(&conn, transformed).await;
  if r.is_none() {
    return Err(Status::InternalServerError);
  }

  Ok(())
}

/// Retrieves a list of albums of an authenticated user
#[openapi]
#[get("/album")]
pub async fn get_album_list(claims: Claims, conn: DbConn) -> Json<Vec<AlbumResponse>> {
  let albums = db::albums::get_album_list(&conn, claims.user_id).await;

  let result = albums.iter()
    .map(AlbumResponse::from)
    .collect::<Vec<AlbumResponse>>();

  Json(result)
}

#[derive(Serialize, Deserialize, JsonSchema, Queryable)]
pub struct AlbumUpdateData {
  pub name: Option<String>,
  pub description: Option<String>,
}

// TODO: rewrite later and use forwarding (ranks)
// problem seems to be in okapi as it overwrites the route when there are multiple ranks
// while the Request guards are wrapped in Option, there are no error codes from that Request guards
/// Gets a list of media in an album
#[openapi]
#[get("/album/<album_uuid>/media")]
pub async fn get_album_structure(shared_album_link_security: Option<SharedAlbumLinkSecurity>, claims_option: Option<Claims>, conn: DbConn, album_uuid: String) -> Result<Json<Vec<MediaResponse>>, Status> {
  let album_id_option = db::albums::select_album_id(&conn, album_uuid).await;
  if album_id_option.is_none() {
    return Err(Status::NotFound);
  }

  let album_option = db::albums::select_album(&conn, album_id_option.unwrap()).await;
  if album_option.is_none() {
    return Err(Status::NotFound);
  }

  let album = album_option.unwrap();

  if claims_option.is_some() {
    if album.owner_id != claims_option.unwrap().user_id {
      return Err(Status::Unauthorized);
    }

    // let accessible = db::albums::user_has_album_access(&conn, claims.user_id, album_id).await;
    // if accessible.is_err() { return Err(Status::InternalServerError) }

    // if !accessible.unwrap() {
    //   return Err(Status::Forbidden);
    // }

    // TODO: check if non-owner user has permission to access the album (preparation for shared albums)

  } else if shared_album_link_security.is_some() {
    // TODO: maybe check more things
  } else {
    return Err(Status::Unauthorized);
  }

  let structure = db::albums::get_album_media(&conn, album.id).await;

  if structure.is_err() { return Err(Status::InternalServerError) }

  let result = structure.unwrap().iter()
    .map(MediaResponse::from)
    .collect::<Vec<MediaResponse>>();

  Ok(Json(result))
}

/// Updates already existing album
#[openapi]
#[put("/album/<album_uuid>", data = "<album_update_data>", format = "json")]
pub async fn update_album(claims: Claims, conn: DbConn, album_uuid: String, album_update_data: Json<AlbumUpdateData>) -> Result<Status, Status> {
  if album_update_data.name.is_none() && album_update_data.description.is_none() {
    return Err(Status::UnprocessableEntity);
  }

  let album_id_option = db::albums::select_album_id(&conn, album_uuid).await;
  if album_id_option.is_none() {
    return Err(Status::NotFound);
  }

  let album_id = album_id_option.unwrap();

  let accessible = db::albums::user_has_album_access(&conn, claims.user_id, album_id).await;
  if accessible.is_err() { return Err(Status::InternalServerError) }

  if !accessible.unwrap() {
    return Err(Status::Forbidden);
  }

  let changed_rows = db::albums::update_album(&conn, album_id, album_update_data.into_inner()).await;
  error!("changed: {:?}", changed_rows);
  if changed_rows.is_none() { return Err(Status::InternalServerError) }

  if changed_rows.unwrap() == 0 {
    return Ok(Status::NoContent);
  }

  Ok(Status::Ok)
}

/// Deletes an album
#[openapi]
#[delete("/album/<album_uuid>")]
pub async fn delete_album(claims: Claims, conn: DbConn, album_uuid: String) -> Result<Status, Status> {
  let album_id_option = db::albums::select_album_id(&conn, album_uuid).await;
  if album_id_option.is_none() {
    return Err(Status::NotFound);
  }

  let album_id = album_id_option.unwrap();

  let album = db::albums::select_album(&conn, album_id).await;

  if album.is_none() { return Err(Status::NotFound); }

  let accessible = db::albums::user_has_album_access(&conn, claims.user_id, album_id).await;
  if accessible.is_err() { return Err(Status::InternalServerError) }

  if !accessible.unwrap() {
    return Err(Status::Forbidden);
  }

  let deleted = db::albums::delete_album(&conn, album_id).await;
  if deleted.is_err() { return Err(Status::ImATeapot) }

  Ok(Status::Ok)
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct AlbumShareLinkInsert {
  pub expiration: Option<NaiveDateTime>,
  pub password: Option<String>,
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct SharedAlbumLinkResponse {
  uuid: String,
  expiration: Option<NaiveDateTime>,
}

/// Creates a new album share link.
#[openapi]
#[post("/album/<album_uuid>/share/link", data = "<album_share_link_insert>", format = "json")]
pub async fn create_album_share_link(claims: Claims, conn: DbConn, album_uuid: String, album_share_link_insert: Json<AlbumShareLinkInsert>) -> Result<Json<SharedAlbumLinkResponse>, Status> {
  let album_id_option = db::albums::select_album_id(&conn, album_uuid).await;
  if album_id_option.is_none() { return Err(Status::NotFound) }

  let album_id = album_id_option.unwrap();

  let album = db::albums::select_album(&conn, album_id).await;
  if album.is_none() { return Err(Status::NotFound) }

  if album.unwrap().owner_id != claims.user_id { return Err(Status::Forbidden) }

  let album_share_link_insert_inner = album_share_link_insert.into_inner();
  let album_share_link = NewAlbumShareLink::new(album_id, album_share_link_insert_inner.password, album_share_link_insert_inner.expiration);

  // It would be better to return result and have different responses for each error kind.
  // But it looks like that Diesel uses one error kind for multiple different errors and changes only the message.
  let changed_rows = db::albums::insert_album_share_link(&conn, album_share_link.clone()).await;
  if changed_rows.is_err() { return Err(Status::InternalServerError) }
  if changed_rows.unwrap() == 0 { return Err(Status::InternalServerError) }

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
#[openapi]
#[get("/album/<album_uuid>/share/link")]
pub async fn get_album_share_links(claims: Claims, conn: DbConn, album_uuid: String) -> Result<Json<Vec<SharedAlbumLinkResponse>>, Status> {
  let album_id_option = db::albums::select_album_id(&conn, album_uuid).await;
  if album_id_option.is_none() {
    return Err(Status::NotFound);
  }

  let album_id = album_id_option.unwrap();

  let album = db::albums::select_album(&conn, album_id).await;
  if album.is_none() { return Err(Status::NotFound) }

  if album.unwrap().owner_id != claims.user_id { return Err(Status::Forbidden) }

  let links = db::albums::select_album_share_links(&conn, album_id).await;
  if links.is_err() { return Err(Status::InternalServerError) }

  let result = links.unwrap().iter()
    .map(SharedAlbumLinkResponse::from)
    .collect::<Vec<SharedAlbumLinkResponse>>();

  Ok(Json(result))
}

#[derive(Serialize, JsonSchema)]
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

/// Gets basic information about album share link.
#[openapi]
#[get("/album/share/link/<album_share_link_uuid>")]
pub async fn get_album_share_link(conn: DbConn, album_share_link_uuid: String) -> Result<Json<AlbumShareLinkBasic>, Status> {
  let album_share_link_result = db::albums::select_album_share_link_by_uuid(&conn, album_share_link_uuid).await;
  if album_share_link_result.is_err() { return Err(Status::InternalServerError) }

  let album_share_link_option = album_share_link_result.unwrap();
  if album_share_link_option.is_none() { return Err(Status::NotFound) }

  let album_share_link = album_share_link_option.unwrap();

  let album = db::albums::select_album(&conn, album_share_link.album_id).await;
  if album.is_none() { return Err(Status::InternalServerError)  }

  Ok(
    Json(
      AlbumShareLinkBasic::new(album_share_link, album.unwrap().link)
    )
  )
}

/// Updates already existing album share link.
#[openapi]
#[put("/album/share/link/<album_share_link_uuid>", data = "<album_share_link_insert>", format = "json")]
pub async fn update_album_share_link(claims: Claims, conn: DbConn, album_share_link_uuid: String, album_share_link_insert: Json<AlbumShareLinkInsert>) -> Result<Status, Status> {
  let album_share_link_result = db::albums::select_album_share_link_by_uuid(&conn, album_share_link_uuid).await;
  if album_share_link_result.is_err() { return Err(Status::InternalServerError) }

  let album_share_link_option = album_share_link_result.unwrap();
  if album_share_link_option.is_none() { return Err(Status::NotFound) }

  let album_share_link = album_share_link_option.unwrap();

  let album = db::albums::select_album(&conn, album_share_link.album_id).await;
  if album.is_none() { return Err(Status::NotFound) }

  if album.unwrap().owner_id != claims.user_id { return Err(Status::Forbidden) }

  let changed_rows = db::albums::update_album_share_link(&conn, album_share_link.id, album_share_link_insert.into_inner()).await;
  if changed_rows.is_err() { return Err(Status::InternalServerError) }

  if changed_rows.unwrap() == 0 {
    return Ok(Status::NoContent);
  }

  Ok(Status::Ok)
}

/// Deletes an album share link.
#[openapi]
#[delete("/album/share/link/<album_share_link_uuid>")]
pub async fn delete_album_share_link(claims: Claims, conn: DbConn, album_share_link_uuid: String) -> Result<Status, Status> {
  let album_share_link_result = db::albums::select_album_share_link_by_uuid(&conn, album_share_link_uuid.clone()).await;
  if album_share_link_result.is_err() { return Err(Status::InternalServerError) }

  let album_share_link = album_share_link_result.unwrap();
  if album_share_link.is_none() { return Err(Status::NotFound) }

  let album = db::albums::select_album(&conn, album_share_link.unwrap().album_id).await;
  if album.is_none() { return Err(Status::NotFound) }

  if album.unwrap().owner_id != claims.user_id { return Err(Status::Forbidden) }

  let deleted = db::albums::delete_album_share_link(&conn, album_share_link_uuid).await;
  if deleted.is_err() { return Err(Status::InternalServerError) }

  if deleted.unwrap() == 0 {
    return Ok(Status::NoContent);
  }

  Ok(Status::Ok)
}

/// Searches for new media
// https://api.rocket.rs/master/rocket/struct.State.html
#[openapi]
#[get("/scan_media")]
pub async fn scan_media(claims: Claims, conn: DbConn) -> &'static str {
  let xdg_data = "gallery";

  // let now_future = Delay::new(Duration::from_secs(10));

  // this thread will run until scanning is complete
  // thread::spawn(|conn, xdg_data, user_id| async {
  scan::scan_root(&conn, xdg_data, claims.user_id).await;
  // });

  "true"
}

// TODO: rewrite later and use forwarding (ranks)
// problem seems to be in okapi as it overwrites the route when there are multiple ranks
// while the Request guards are wrapped in Option, there are no error codes from that Request guards
/// Returns a media
#[openapi]
#[get("/media/<media_uuid>")]
pub async fn get_media_by_uuid(shared_album_link_security: Option<SharedAlbumLinkSecurity>, claims_option: Option<Claims>, conn: DbConn, media_uuid: String) -> Option<NamedFile> {
  let media: Media = conn.run(|c| {
    media::table
      .select(media::table::all_columns())
      .filter(media::dsl::uuid.eq(media_uuid))
      .first::<Media>(c)
      .optional()
      .unwrap()
  }).await?;

  if claims_option.is_some() {
    if media.owner_id != claims_option.unwrap().user_id {
      return None;
    }

    // TODO: check if non-owner user has permission to access the album (preparation for shared albums)

  } else if shared_album_link_security.is_some() {
    // TODO: maybe check more things
  } else {
    return None;
  }

  let xdg_data = "gallery";

  let mut folders: Vec<Folder> = vec!();

  let current_folder = db::folders::select_folder(&conn, media.folder_id).await?;
  folders.push(current_folder.clone());

  scan::select_parent_folder_recursive(&conn, current_folder, media.owner_id, &mut folders);

  let mut path = format!("{}/{}/", xdg_data, db::users::get_user_username(&conn, media.owner_id).await?);

  if !folders.is_empty() {
    for folder in folders.iter().rev() {
      path += format!("{}/", folder.name).as_str();
    }
  }
  path += &media.filename;

  NamedFile::open(path).await.ok()
}

#[derive(Serialize, Deserialize, JsonSchema)]
pub struct MediaDescription {
  description: Option<String>
}

/// Updates description of a media
#[openapi]
#[put("/media/<media_uuid>/description", data = "<description>", format = "json")]
pub async fn media_update_description(claims: Claims, conn: DbConn, media_uuid: String, description: Json<MediaDescription>) -> Result<Status, Status> {
  let media_id_option = db::media::select_media_id(&conn, media_uuid.clone()).await;
  if media_id_option.is_none() {
    return Err(Status::NotFound);
  }

  let access = db::media::media_user_has_access(&conn, media_uuid, claims.user_id).await;
  if access.is_err() { return Err(Status::InternalServerError) }

  if !access.unwrap() { return Err(Status::Forbidden) }

  let media_id = media_id_option.unwrap();

  let mut description_option = description.into_inner().description;

  // check for empty string
  if let Some(string) = description_option {
    description_option = if string.chars().count() > 0 { Some(string) } else { None };
  }

  let result = db::media::update_description(&conn, media_id, description_option).await;

  if result.is_err() { return Err(Status::InternalServerError) }

  Ok(Status::Ok)
}

/// Deletes description of a media
#[openapi]
#[delete("/media/<media_uuid>/description")]
pub async fn media_delete_description(claims: Claims, conn: DbConn, media_uuid: String) -> Result<Status, Status> {
  let media_id_option = db::media::select_media_id(&conn, media_uuid.clone()).await;
  if media_id_option.is_none() {
    return Err(Status::NotFound);
  }

  let access = db::media::media_user_has_access(&conn, media_uuid, claims.user_id).await;
  if access.is_err() { return Err(Status::InternalServerError) }

  if !access.unwrap() { return Err(Status::Forbidden) }

  let media_id = media_id_option.unwrap();

  let result = db::media::update_description(&conn, media_id, None).await;

  if result.is_err() { return Err(Status::InternalServerError) }

  Ok(Status::Ok)
}

/// Returns a list of liked media.
#[openapi]
#[get("/media/liked")]
pub async fn get_media_liked_list(claims: Claims, conn: DbConn) -> Result<Json<Vec<MediaResponse>>, Status> {
  let liked = db::media::get_liked_media(&conn, claims.user_id).await;

  if liked.is_err() {
    return Err(Status::InternalServerError)
  }

  let result = liked.unwrap().iter()
    .map(MediaResponse::from)
    .collect::<Vec<MediaResponse>>();

  Ok(Json(result))
}

/// Likes the media.
#[openapi]
#[post("/media/<media_uuid>/like")]
pub async fn media_like(claims: Claims, conn: DbConn, media_uuid: String) -> Result<Status, Status> {
  let media_id_option = db::media::select_media_id(&conn, media_uuid).await;
  if media_id_option.is_none() {
    return Err(Status::NotFound);
  }

  let media_id = media_id_option.unwrap();

  // It would be better to return result and have different responses for each error kind.
  // But it looks like that Diesel uses one error kind for multiple different errors and changes only the message.
  let changed_rows = db::media::media_like(&conn, media_id, claims.user_id).await;
  if changed_rows.is_ok() {
    return Ok(Status::Ok);
  }

  error!("Inserting like failed: {}", changed_rows.unwrap_err());
  Err(Status::Conflict)
}

/// Unlikes the media.
#[openapi]
#[delete("/media/<media_uuid>/like")]
pub async fn media_unlike(claims: Claims, conn: DbConn, media_uuid: String) -> Result<Status, Status> {
  let media_id_option = db::media::select_media_id(&conn, media_uuid).await;
  if media_id_option.is_none() {
    return Err(Status::NotFound);
  }

  let media_id = media_id_option.unwrap();

  let r = db::media::media_unlike(&conn, media_id, claims.user_id).await;

  if r.is_err() { return Ok(Status::InternalServerError) }

  let changed_rows = r.unwrap();

  if changed_rows == 0 {
    return Ok(Status::NoContent);
  }

  Ok(Status::Ok)
}

#[derive(Serialize, Deserialize, JsonSchema)]
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

/// Returns the public system information.
#[openapi]
#[get("/system/info/public")]
pub async fn system_info_public() -> Json<SystemInfoPublic> {
  Json(SystemInfoPublic::new())
}
