use crate::models::*;
use crate::schema::{favorite_media, media};
use crate::DbConn;
use checksums::{hash_file, Algorithm::SHA2512};
use chrono::NaiveDateTime;
use diesel::BoolExpressionMethods;
use diesel::ExpressionMethods;
use diesel::OptionalExtension;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use diesel::Table;
use std::path::PathBuf;
use uuid::Uuid;

/// Checks whether a specific file is already present in a database.
/// # Example
/// We have a picture named cat.jpg and we need to check if it's already in a database.
/// ```
/// let media: Option<i32> = check_if_media_present(&conn, name, parent_folder, user_id).await;
/// ```
pub async fn check_if_media_present(conn: &DbConn, name: String, parent_folder: Folder, user_id: i32) -> Option<i32> {
  conn.run(move |c| {
    return media::table
      .select(media::id)
      .filter(media::dsl::filename.eq(name).and(media::owner_id.eq(user_id).and(media::folder_id.eq(parent_folder.id))))
      .first::<i32>(c)
      .optional()
      .unwrap();
  }).await
}

/// Inserts new media.
pub async fn insert_media(conn: &DbConn, name: String, parent_folder: Folder, user_id: i32, image_dimensions: (u32, u32), media_scanned: PathBuf) {
  conn.run(move |c| {
    // error!("file {} doesnt exist", name.display().to_string());
    let uuid = Uuid::new_v4().to_string();
    let new_media = NewMedia::new(name.clone(), parent_folder.id, user_id, image_dimensions.0, image_dimensions.1, NaiveDateTime::from_timestamp(10, 10), uuid, hash_file(&media_scanned, SHA2512));
    let insert = diesel::insert_into(media::table)
      .values(new_media)
      .execute(c)
      .expect(format!("Error inserting file {:?}", name).as_str());

    return insert;
  }).await;
}

pub async fn get_media_structure(conn: &DbConn, user_id: i32) -> Vec<crate::routes::MediaResponse> {
  let structure: Vec<Media> = conn.run(move |c| {
    return media::table
      .select(media::table::all_columns())
      .filter(media::owner_id.eq(user_id))
      .load::<Media>(c)
      .unwrap()
  }).await;

  let mut vec: Vec<crate::routes::MediaResponse> = vec!();

  for response in structure {
    vec.push(
      crate::routes::MediaResponse { filename: response.filename, owner_id: response.owner_id, width: response.width, height: response.height, date_taken: response.date_taken, uuid: response.uuid }
    )
  }

  return vec;
}

pub async fn select_media_id(conn: &DbConn, media_uuid: String) -> Option<i32> {
  conn.run(move |c| {
    media::table
      .select(media::id)
      .filter(media::dsl::uuid.eq(media_uuid))
      .first::<i32>(c)
      .optional()
      .unwrap()
  }).await
}

pub async fn media_like(conn: &DbConn, media_id: i32, user_id: i32) -> Result<usize, diesel::result::Error> {
  let new_like = NewFavoriteMedia::new(media_id, user_id);
  conn.run(move |c| {
    diesel::insert_into(favorite_media::table)
      .values(new_like)
      .execute(c)
  }).await
}

pub async fn media_unlike(conn: &DbConn, media_id: i32, user_id: i32) -> Result<usize, diesel::result::Error> {
  conn.run(move |c| {
    diesel::delete(
      favorite_media::table
        .filter(favorite_media::media_id.eq(media_id).and(favorite_media::user_id.eq(user_id)))
    )
      .execute(c)
  }).await
}

pub async fn get_liked_media(conn: &DbConn, user_id: i32) -> Result<Vec<Media>, diesel::result::Error> {
  conn.run(move |c| {
    media::table
      .select(media::table::all_columns())
      .filter(media::id.eq_any(
        favorite_media::table
          .select(favorite_media::media_id)
          .filter(favorite_media::user_id.eq(user_id))
      ))
      .get_results::<Media>(c)
  }).await
}
