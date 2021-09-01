use crate::auth::Claims;
use crate::db;
use crate::models::{self, *};
use crate::scan;
use crate::schema::media;
use crate::DbConn;
use diesel::ExpressionMethods;
use diesel::OptionalExtension;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use diesel::Table;
use futures::executor;
use rocket::fs::NamedFile;
use serde::{Deserialize, Serialize};
use schemars::JsonSchema;
use rocket::serde::json::Json;
use sha2::{self, Digest};

#[openapi]
#[get("/")]
pub async fn index() -> &'static str {
  "Hello, world!"
}

#[openapi]
#[post("/user", data = "<user>", format = "json")]
pub async fn create_user(conn: DbConn, user: Json<NewUser>) -> Json<bool> {
  if !db::users::is_user_unique(&conn, user.0.clone()).await { return Json(false); };

  let mut hasher = sha2::Sha512::new();
  hasher.update(user.0.password);
  // {:x} means format as hexadecimal
  let hashed_password = format!("{:X}", hasher.finalize());

  let new_user = NewUser { username: user.0.username, email: user.0.email, password: hashed_password };
  let result = db::users::insert_user(&conn, new_user.clone()).await;
  if result == 0 { return Json(false) }

  info!("A new user was created with name {}", new_user.username);
  Json(true)
}

#[derive(Serialize, Deserialize, JsonSchema, Queryable)]
pub struct MediaResponse {
  pub filename: String,
  pub owner_id: i32,
  pub album_id: Option<i32>,
  pub width: u32,
  pub height: u32,
  pub date_taken: chrono::NaiveDateTime,
  pub uuid: String,
}

// FIXME: skips new media in /gallery/username/<medianame>; /gallery/username/<some_folder>/<medianame> works
#[openapi]
#[get("/media")]
pub async fn media_structure(claims: Claims, conn: DbConn) -> Json<Vec<MediaResponse>> {
  error!("user_id: {}", claims.user_id);

  let structure = db::media::get_media_structure(&conn, claims.user_id).await;

  Json(structure)
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
  let media_option: Option<models::Media> = conn.run(|c| {
    return crate::schema::media::table
      .select(crate::schema::media::table::all_columns())
      .filter(media::dsl::uuid.eq(media_uuid))
      .first::<Media>(c)
      .optional()
      .unwrap();
  }).await;

  if media_option.is_none() { return None; }

  let media = media_option.unwrap();

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
