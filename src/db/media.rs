use crate::models::*;
use crate::schema::{favorite_media, media};
use crate::routes::media::MediaResponse;
use crate::DbConn;
use checksums::{hash_file, Algorithm::SHA2512};
use chrono::NaiveDateTime;
use diesel::BoolExpressionMethods;
use diesel::ExpressionMethods;
use diesel::OptionalExtension;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use diesel::Table;
use tracing::error;
use std::path::PathBuf;
use uuid::Uuid;

/// Checks whether a specific file is already present in a database.
/// # Example
/// We have a picture named cat.jpg and we need to check if it's already in a database.
/// ```
/// let media: Option<i32> = check_if_media_present(&conn, name, parent_folder, user_id).await;
/// ```
pub async fn check_if_media_present(conn: DbConn, name: String, parent_folder: Folder, user_id: i32) -> Option<i32> {
  conn.interact(move |c| {
    media::table
      .select(media::id)
      .filter(media::dsl::filename.eq(name).and(media::owner_id.eq(user_id).and(media::folder_id.eq(parent_folder.id))))
      .first::<i32>(c)
      .optional()
      .unwrap()
  }).await.unwrap()
}

/// Inserts new media.
pub async fn insert_media(conn: DbConn, name: String, parent_folder: Folder, user_id: i32, image_dimensions: (u32, u32), description: Option<String>, media_scanned: PathBuf) {
  conn.interact(move |c| {
    let uuid = Uuid::new_v4().to_string();
    let new_media = NewMedia::new(name.clone(), parent_folder.id, user_id, image_dimensions.0, image_dimensions.1, description, NaiveDateTime::from_timestamp(10, 10), uuid, hash_file(&media_scanned, SHA2512));

    diesel::insert_into(media::table)
      .values(new_media)
      .execute(c)
      .unwrap_or_else(|_| panic!("Error inserting file {:?}", name))
  }).await.unwrap();
}

/// Returns a skeleton media list.
pub async fn get_media_structure(conn: DbConn, user_id: i32) -> Vec<MediaResponse> {
  let structure: Vec<Media> = conn.interact(move |c| {
    media::table
      .select(media::table::all_columns())
      .filter(media::owner_id.eq(user_id))
      .load::<Media>(c)
      .unwrap()
  }).await.unwrap();

  let mut vec: Vec<MediaResponse> = vec!();

  for response in structure {
    vec.push(
      MediaResponse::from(response)
    )
  }

  vec
}

/// Tries to select a media ID from its UUID.
pub async fn select_media_id(conn: DbConn, media_uuid: String) -> Option<i32> {
  conn.interact(move |c| {
    media::table
      .select(media::id)
      .filter(media::dsl::uuid.eq(media_uuid))
      .first::<i32>(c)
      .optional()
      .unwrap()
  }).await.unwrap()
}

pub async fn select_media_by_uuid(conn: DbConn, media_uuid: String) -> Result<Option<Media>, diesel::result::Error> {
  let result = conn.interact(|c| {
    media::table
      .select(media::table::all_columns())
      .filter(media::dsl::uuid.eq(media_uuid))
      .first::<Media>(c)
      .optional()
    }).await
  .map_err(|e| {
    error!("DB interact failed in select_media_by_uuid: {e}");
    diesel::result::Error::DatabaseError(
      diesel::result::DatabaseErrorKind::Unknown,
      Box::new(format!("interact failed: {e}")),
    )
  })??;

  Ok(result)
}

/// Checks whether a user has access to the media.
// TODO: check more places for permissions
pub async fn media_user_has_access(conn: DbConn, media_uuid: String, owner_id: i32) -> Result<bool, diesel::result::Error> {
  conn.interact(move |c| {
    diesel::dsl::select(
        diesel::dsl::exists(
          media::table.filter(
            media::dsl::uuid.eq(media_uuid).and(media::dsl::owner_id.eq(owner_id))
          )
        )
      )
      .get_result(c)
  }).await.unwrap()
}

/// Likes the media.
pub async fn media_like(conn: DbConn, media_id: i32, user_id: i32) -> Result<usize, diesel::result::Error> {
  let new_like = NewFavoriteMedia::new(media_id, user_id);
  conn.interact(move |c| {
    diesel::insert_into(favorite_media::table)
      .values(new_like)
      .execute(c)
  }).await.unwrap()
}

/// Unlikes the media.
pub async fn media_unlike(conn: DbConn, media_id: i32, user_id: i32) -> Result<usize, diesel::result::Error> {
  conn.interact(move |c| {
    diesel::delete(
      favorite_media::table
        .filter(favorite_media::media_id.eq(media_id).and(favorite_media::user_id.eq(user_id)))
    )
      .execute(c)
  }).await.unwrap()
}

/// Gets a list of liked media.
pub async fn get_liked_media(conn: DbConn, user_id: i32) -> Result<Vec<Media>, diesel::result::Error> {
  conn.interact(move |c| {
    media::table
      .select(media::table::all_columns())
      .filter(media::id.eq_any(
        favorite_media::table
          .select(favorite_media::media_id)
          .filter(favorite_media::user_id.eq(user_id))
      ))
      .get_results::<Media>(c)
  }).await.unwrap()
}

/// Updates media description.
pub async fn update_description(conn: DbConn, media_id: i32, description: Option<String>) -> Result<usize, diesel::result::Error> {
  conn.interact(move |c| {
    diesel::update(media::table.filter(media::id.eq(media_id)))
      .set(media::dsl::description.eq(description))
      .execute(c)
  }).await.unwrap()
}
