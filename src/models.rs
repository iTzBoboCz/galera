use super::schema::{user, folder, album, album_invite, photo, favourite_photo};
use diesel::sql_types::Timestamp;

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable)]
#[table_name = "user"]
pub struct User {
  pub id: i32,
  pub username: String,
  pub email: String,
}

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable, Associations)]
#[table_name = "folder"]
#[belongs_to(User, foreign_key = "owner_id")]
#[belongs_to(Folder, foreign_key = "parent")]
pub struct Folder {
  pub id: i32,
  pub owner_id: i32,
  pub parent: Option<i32>,
  pub name: String,
}

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable, Associations)]
#[table_name = "album"]
#[belongs_to(User, foreign_key = "owner_id")]
pub struct Album {
  pub id: i32,
  pub owner_id: i32,
  pub link: Option<String>,
  pub password: Option<String>,
}

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable, Associations)]
#[table_name = "album_invite"]
#[belongs_to(Album, foreign_key = "album_id")]
#[belongs_to(User, foreign_key = "invited_user_id")]
pub struct Album_invite {
  pub id: i32,
  pub album_id: i32,
  pub invited_user_id: i32,
  pub accepted: bool,
  pub write_access: bool,
}

//#[table_name = "posts"]
#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable, Associations)]
#[table_name = "photo"]
#[belongs_to(Folder, foreign_key = "folder_id")]
#[belongs_to(User, foreign_key = "owner_id")]
pub struct Photo {
  pub id: i32,
  pub filename: String,
  pub folder_id: i32,
  pub owner_id: i32,
  pub album_id: Option<i32>,
  pub width: i32,
  pub height: i32,
  pub date_taken: Timestamp,
  pub sha2_512_hash: String,
}

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable, Associations)]
#[table_name = "favourite_photo"]
#[belongs_to(Photo, foreign_key = "photo_id")]
#[belongs_to(User, foreign_key = "user_id")]
pub struct Favourite_photo {
  pub id: i32,
  pub photo_id: i32,
  pub user_id: i32,
}

// https://github.com/diesel-rs/diesel/issues/616
// https://stackoverflow.com/questions/56853059/use-of-undeclared-type-or-module-when-using-diesels-belongs-to-attribute
