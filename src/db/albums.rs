use crate::models::{Album, AlbumShareLink, Media, NewAlbum, NewAlbumMedia, NewAlbumShareLink};
use crate::routes::{AlbumInsertData, AlbumShareLinkInsert, AlbumUpdateData};
use crate::schema::{album, album_media, album_share_link, media};
use crate::DbConn;
use diesel::BoolExpressionMethods;
use diesel::ExpressionMethods;
use diesel::OptionalExtension;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use diesel::Table;

// Checks whether the user has access to the album.
pub async fn user_has_album_access(conn: DbConn, user_id: i32, album_id: i32) -> Result<bool, diesel::result::Error> {
  let id: Option<i32> = conn.interact(move |c| {
    album::table
      .select(album::dsl::id)
      .filter(album::dsl::id.eq(album_id).and(album::dsl::owner_id.eq(user_id)))
      .first::<i32>(c)
      .optional()
  }).await.unwrap()?;

  if id.is_none() {
    return Ok(false);
  }

  Ok(true)
}

pub async fn select_album(conn: DbConn, album_id: i32) -> Option<Album> {
  conn.interact(move |c| {
    album::table
      .select(album::table::all_columns())
      .filter(album::dsl::id.eq(album_id))
      .first::<Album>(c)
      .optional()
      .unwrap()
  }).await.unwrap()
}

pub async fn select_album_id(conn: DbConn, album_uuid: String) -> Option<i32> {
  conn.interact(move |c| {
    album::table
      .select(album::id)
      .filter(album::dsl::link.eq(album_uuid))
      .first::<i32>(c)
      .optional()
      .unwrap()
  }).await.unwrap()
}

pub async fn insert_album(conn: DbConn, user_id: i32, album_insert_data: AlbumInsertData) {
  let new_album = NewAlbum::new(user_id, album_insert_data.name, album_insert_data.description, None);
  conn.interact(move |c| {
    diesel::insert_into(album::table)
      .values(new_album.clone())
      .execute(c)
      .unwrap_or_else(|_| panic!("Could not add a new album for user with ID {}", new_album.owner_id));
  }).await.unwrap();
}

pub async fn get_album_list(conn: DbConn, user_id: i32) -> Vec<Album> {
  conn.interact(move |c| {
    album::table
      .select(album::table::all_columns())
      .filter(album::dsl::owner_id.eq(user_id))
      .get_results::<Album>(c)
      .optional()
      .unwrap()
      .unwrap()
  }).await.unwrap()
}

pub async fn album_add_media(conn: DbConn, list_of_media: Vec<NewAlbumMedia>) -> Option<()> {
  let r: Result<usize, diesel::result::Error> = conn.interact(move |c| {
    diesel::insert_into(album_media::table)
      .values(list_of_media)
      .execute(c)
  }).await.unwrap();

  if r.is_err() {
    return None;
  }

  Some(())
}

pub async fn album_already_has_media(conn: DbConn, album_id: i32, media_id: i32) -> Result<bool, diesel::result::Error> {
  let id: Result<Option<i32>, diesel::result::Error> = conn.interact(move |c| {
    album_media::table
    .select(album_media::id)
    .filter(album_media::dsl::album_id.eq(album_id).and(album_media::dsl::media_id.eq(media_id)))
    .first::<i32>(c)
    .optional()
  }).await.unwrap();

  if id.is_err() {
    return Err(id.unwrap_err())
  } else {
    let id_unwrapped = id.unwrap();
    if id_unwrapped.is_some() {
      Ok(true)
    } else {
      Ok(false)
    }
  }
}

pub async fn update_album(conn: DbConn, album_id: i32, album_update_data: AlbumUpdateData) -> Option<usize> {
  let mut name_result: Result<usize, diesel::result::Error> = Ok(0);
  let mut description_result: Result<usize, diesel::result::Error> = Ok(0);

  let name = album_update_data.name;
  let description = album_update_data.description;

  if name.is_some() {
    name_result = conn.interact(move |c| {
      diesel::update(album::table.filter(album::id.eq(album_id)))
        .set(album::dsl::name.eq(name.unwrap()))
        .execute(c)
    }).await.unwrap();
  }

  if description.is_some() {
    description_result = conn.interact(move |c| {
      diesel::update(album::table.filter(album::id.eq(album_id)))
        .set(album::dsl::description.eq(description.unwrap()))
        .execute(c)
    }).await.unwrap();
  }

  if name_result.is_err() || description_result.is_err() {
    return None;
  }

  Some(name_result.unwrap() + description_result.unwrap())
}

pub async fn delete_album(conn: DbConn, album_id: i32) -> Result<usize, diesel::result::Error> {
  conn.interact(move |c| {
    diesel::delete(album::table.filter(album::id.eq(album_id)))
      .execute(c)
  }).await.unwrap()
}

pub async fn get_album_media(conn: DbConn, album_id: i32) -> Result<Vec<Media>, diesel::result::Error> {
  conn.interact(move |c| {
    media::table
      .select(media::table::all_columns())
      .filter(media::id.eq_any(
        album_media::table
          .select(album_media::media_id)
          .filter(album_media::album_id.eq(album_id))
      ))
      .get_results::<Media>(c)
  }).await.unwrap()
}

pub async fn select_album_share_links(conn: DbConn, album_id: i32) -> Result<Vec<AlbumShareLink>, diesel::result::Error> {
  conn.interact(move |c| {
    album_share_link::table
      .select(album_share_link::table::all_columns())
      .filter(album_share_link::dsl::album_id.eq(album_id))
      .get_results::<AlbumShareLink>(c)
  }).await.unwrap()
}

pub async fn select_album_share_link_by_link(conn: DbConn, album_share_link_link: String) -> Result<Option<AlbumShareLink>, diesel::result::Error> {
  conn.interact(move |c| {
    album_share_link::table
      .select(album_share_link::table::all_columns())
      .filter(album_share_link::dsl::link.eq(album_share_link_link))
      .first::<AlbumShareLink>(c)
      .optional()
  }).await.unwrap()
}

pub async fn select_album_share_link_by_uuid(conn: DbConn, album_share_link_uuid: String) -> Result<Option<AlbumShareLink>, diesel::result::Error> {
  conn.interact(move |c| {
    album_share_link::table
      .select(album_share_link::table::all_columns())
      .filter(album_share_link::dsl::uuid.eq(album_share_link_uuid))
      .first::<AlbumShareLink>(c)
      .optional()
  }).await.unwrap()
}

pub async fn insert_album_share_link(conn: DbConn, album_share_link: NewAlbumShareLink) -> Result<usize, diesel::result::Error> {
  conn.interact(move |c| {
    diesel::insert_into(album_share_link::table)
      .values(album_share_link)
      .execute(c)
  }).await.unwrap()
}

/// Updates album share link.
pub async fn update_album_share_link(conn: DbConn, album_share_link_id: i32, album_share_link_insert: AlbumShareLinkInsert) -> Result<usize, diesel::result::Error> {
  conn.interact(move |c| {
    diesel::update(album_share_link::table.filter(album_share_link::id.eq(album_share_link_id)))
      .set(
        (album_share_link::dsl::expiration.eq(album_share_link_insert.expiration),
        album_share_link::dsl::password.eq(album_share_link_insert.password)))
      .execute(c)
  }).await.unwrap()
}

/// Removes album share link.
pub async fn delete_album_share_link(conn: DbConn, album_share_link_uuid: String) -> Result<usize, diesel::result::Error> {
  conn.interact(move |c| {
    diesel::delete(
      album_share_link::table
        .filter(album_share_link::uuid.eq(album_share_link_uuid))
    )
      .execute(c)
  }).await.unwrap()
}
