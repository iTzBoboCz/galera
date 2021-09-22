use crate::models::{self, *};
use crate::routes::{AlbumInsertData, AlbumUpdateData};
use crate::schema::{album, album_media};
use crate::DbConn;
use diesel::BoolExpressionMethods;
use diesel::ExpressionMethods;
use diesel::OptionalExtension;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use diesel::Table;

// Checks whether the user has access to the album.
pub async fn user_has_album_access(conn: &DbConn, user_id: i32, album_id: i32) -> Result<bool, diesel::result::Error> {
  let id: Option<i32> = conn.run(move |c| {
    album::table
      .select(album::dsl::id)
      .filter(album::dsl::id.eq(album_id).and(album::dsl::owner_id.eq(user_id)))
      .first::<i32>(c)
      .optional()
  }).await?;

  if id.is_none() {
    return Ok(false);
  }

  Ok(true)
}

pub async fn select_album(conn: &DbConn, album_id: i32) -> Option<Album> {
  conn.run(move |c| {
    album::table
      .select(album::table::all_columns())
      .filter(album::dsl::id.eq(album_id))
      .first::<Album>(c)
      .optional()
      .unwrap()
  }).await
}

pub async fn insert_album(conn: &DbConn, user_id: i32, album_insert_data: AlbumInsertData) {
  let new_album = NewAlbum::new(user_id, album_insert_data.name, album_insert_data.description, None);
  conn.run(move |c| {
    let insert = diesel::insert_into(album::table)
      .values(new_album.clone())
      .execute(c)
      .expect(format!("Could not add a new album for user with ID {}", new_album.owner_id).as_str());

    return insert;
  }).await;
}

pub async fn get_album_list(conn: &DbConn, user_id: i32) -> Vec<Album> {
  conn.run(move |c| {
    album::table
      .select(album::table::all_columns())
      .filter(album::dsl::owner_id.eq(user_id))
      .get_results::<Album>(c)
      .optional()
      .unwrap()
      .unwrap()
  }).await
}

pub async fn album_add_media(conn: &DbConn, list_of_media: Vec<NewAlbumMedia>) -> Option<()> {
  let r: Result<usize, diesel::result::Error> = conn.run(move |c| {
    diesel::insert_into(album_media::table)
      .values(list_of_media)
      .execute(c)
  }).await;

  if r.is_err() {
    return None;
  }

  Some(())
}

pub async fn update_album(conn: &DbConn, album_id: i32, album_update_data: AlbumUpdateData) -> Option<usize> {
  let mut name_result: Result<usize, diesel::result::Error> = Ok(0);
  let mut description_result: Result<usize, diesel::result::Error> = Ok(0);

  let name = album_update_data.name;
  let description = album_update_data.description;

  if name.is_some() {
    name_result = conn.run(move |c| {
      diesel::update(album::table.filter(album::id.eq(album_id)))
        .set(album::dsl::name.eq(name.unwrap()))
        .execute(c)
    }).await;
  }

  if description.is_some() {
    description_result = conn.run(move |c| {
      diesel::update(album::table.filter(album::id.eq(album_id)))
        .set(album::dsl::description.eq(description.unwrap()))
        .execute(c)
    }).await;
  }

  if name_result.is_err() || description_result.is_err() {
    return None;
  }

  Some(name_result.unwrap() + description_result.unwrap())
}

pub async fn delete_album(conn: &DbConn, album_id: i32) -> Result<usize, diesel::result::Error> {
  conn.run(move |c| {
    diesel::delete(album::table.filter(album::id.eq(album_id)))
      .execute(c)
  }).await
}
