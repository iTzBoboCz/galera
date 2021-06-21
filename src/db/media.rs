use crate::DbConn;
use crate::models::*;
use crate::schema::media;
use checksums::{ hash_file, Algorithm::SHA2512 };
use diesel::BoolExpressionMethods;
use diesel::ExpressionMethods;
use diesel::RunQueryDsl;
use diesel::QueryDsl;
use diesel::OptionalExtension;
use chrono::NaiveDateTime;
use uuid::Uuid;
use std::path::PathBuf;

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
pub async fn insert_media(conn: &DbConn, name: String, parent_folder: Folder, media_scanned: PathBuf, user_id: i32) {
  conn.run(move |c| {
    // error!("file {} doesnt exist", name.display().to_string());
    let uuid = Uuid::new_v4().to_string();
    let new_media = NewMedia::new(name.clone(), parent_folder.id, user_id, None, 0, 0, NaiveDateTime::from_timestamp(10, 10), uuid, hash_file(&media_scanned, SHA2512));
    let insert = diesel::insert_into(media::table)
      .values(new_media)
      .execute(c)
      .expect(format!("Error inserting file {:?}", name).as_str());

    return insert;
  }).await;
}
