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

#[openapi]
#[get("/")]
pub async fn index() -> &'static str {
  "Hello, world!"
}

#[derive(Serialize, Deserialize, JsonSchema, Queryable)]
pub struct MediaResponse {
  pub filename: String,
  pub owner_id: i32,
  pub album_id: Option<i32>,
  pub width: u32,
  pub height: u32,
  pub date_taken: String,
  pub uuid: String,
}

#[openapi]
#[get("/media")]
pub async fn media_structure(conn: DbConn) -> Json<Vec<MediaResponse>> {
  let user_id: i32 = 1;

  let structure = db::media::get_media_structure(&conn, user_id).await;

  Json(structure)
}

// https://api.rocket.rs/master/rocket/struct.State.html
#[openapi]
#[get("/scan_media")]
pub async fn scan_media(conn: DbConn) -> &'static str {
  let xdg_data = "gallery";
  let user_id: i32 = 1;

  // let now_future = Delay::new(Duration::from_secs(10));

  // this thread will run until scanning is complete
  // thread::spawn(|conn, xdg_data, user_id| async {
  scan::scan_root(&conn, xdg_data, user_id).await;
  // });

  "true"
}

#[openapi]
#[get("/media/<media_uuid>")]
pub async fn get_media_by_uuid(conn: DbConn, media_uuid: String) -> Option<NamedFile> {
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
  let user_id = 1;

  let mut folders: Vec<Folder> = vec!();

  let current_folder = executor::block_on(db::folders::select_folder(&conn, media.folder_id));
  if current_folder.is_none() { return None; }
  folders.push(current_folder.clone().unwrap());

  scan::select_parent_folder_recursive(&conn, current_folder.unwrap(), user_id, &mut folders);

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
