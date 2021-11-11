use crate::auth::login::{UserLogin, UserInfo, LoginResponse};
use crate::auth::token::{Claims, ClaimsEncoded};
use crate::db;
use crate::db::users::get_user_by_id;
use crate::models::{self, *};
use crate::scan;
use crate::schema::media;
use crate::DbConn;
use chrono::NaiveDateTime;
use diesel::ExpressionMethods;
use diesel::OptionalExtension;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use diesel::Table;
use futures::executor;
use rocket::{fs::NamedFile, http::Status};
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use rocket::serde::json::Json;

#[openapi]
#[get("/")]
pub async fn index() -> &'static str {
  "Hello, world!"
}

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

#[openapi]
#[post("/login/refresh")]
pub async fn refresh_token(conn: DbConn, bearer_token_option: Option<Claims>) -> Result<Json<ClaimsEncoded>, Status> {
  if bearer_token_option.is_none() { return Err(Status::UnprocessableEntity); }
  let bearer_token = bearer_token_option.unwrap();

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
  pub date_taken: chrono::NaiveDateTime,
  pub uuid: String,
}

impl From<Media> for MediaResponse {
  fn from(media: Media) -> Self {
    MediaResponse { filename: media.filename, owner_id: media.owner_id, width: media.width, height: media.height, date_taken: media.date_taken, uuid: media.uuid }
  }
}

impl From<&Media> for MediaResponse {
  fn from(media: &Media) -> Self {
    MediaResponse { filename: media.filename.clone(), owner_id: media.owner_id, width: media.width, height: media.height, date_taken: media.date_taken, uuid: media.uuid.clone() }
  }
}

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

#[openapi]
#[post("/album/media", data = "<list_of_media>", format = "json")]
pub async fn album_add_media(claims: Claims, conn: DbConn, list_of_media: Json<Vec<NewAlbumMedia>>) -> Result<(), Status> {
  // TODO: check media and album access
  let r = db::albums::album_add_media(&conn, list_of_media.into_inner()).await;
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
    .map(|r| AlbumResponse::from(r))
    .collect::<Vec<AlbumResponse>>();

  Json(result)
}

#[derive(Serialize, Deserialize, JsonSchema, Queryable)]
pub struct AlbumUpdateData {
  pub name: Option<String>,
  pub description: Option<String>,
}

#[openapi]
#[get("/album/<album_uuid>/media")]
pub async fn get_album_structure(claims: Claims, conn: DbConn, album_uuid: String) -> Result<Json<Vec<MediaResponse>>, Status> {
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

  let structure = db::albums::get_album_media(&conn, album_id).await;

  if structure.is_err() { return Err(Status::InternalServerError) }

  let result = structure.unwrap().iter()
    .map(|r| MediaResponse::from(r))
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

/// Creates a new album
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

#[openapi]
#[get("/media/<media_uuid>")]
pub async fn get_media_by_uuid(claims: Claims, conn: DbConn, media_uuid: String) -> Option<NamedFile> {
  let media: models::Media = conn.run(|c| {
    return crate::schema::media::table
      .select(crate::schema::media::table::all_columns())
      .filter(media::dsl::uuid.eq(media_uuid))
      .first::<Media>(c)
      .optional()
      .unwrap();
  }).await?;

  let xdg_data = "gallery";

  let mut folders: Vec<Folder> = vec!();

  let current_folder = executor::block_on(db::folders::select_folder(&conn, media.folder_id));
  if current_folder.is_none() { return None; }
  folders.push(current_folder.clone().unwrap());

  scan::select_parent_folder_recursive(&conn, current_folder.unwrap(), claims.user_id, &mut folders);

  let relative_path = format!("{}/{}/", xdg_data, db::users::get_user_username(&conn, media.owner_id).await.unwrap());

  let mut path = relative_path;

  if folders.len() > 0 {
    for folder in folders.iter().rev() {
      path += format!("{}/", folder.name).as_str();
    }
  }
  path += &media.filename;

  NamedFile::open(path).await.ok()
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
    .map(|r| MediaResponse::from(r))
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
  return Err(Status::Conflict);
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

#[openapi]
#[get("/test")]
pub async fn test(conn: DbConn) -> String {
  let media: i32 = conn.run(|c| {
  // check wheter the file is already in a database
  return crate::schema::media::table
    .select(crate::schema::media::id)
    .first::<i32>(c)
    .unwrap();
  }).await;

  media.to_string()
}
