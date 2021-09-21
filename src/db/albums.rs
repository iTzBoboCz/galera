use crate::models::{self, *};
use crate::routes::AlbumInsertData;
use crate::schema::{album, album_media};
use crate::DbConn;
use diesel::ExpressionMethods;
use diesel::OptionalExtension;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use diesel::Table;
use nanoid::nanoid;

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
  let new_album = NewAlbum::new(user_id, album_insert_data.name, album_insert_data.description, nanoid!(), None);
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

pub async fn album_add_media(conn: &DbConn, album_id: i32, list_of_media: Vec<NewAlbumMedia>) -> Option<()> {
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
