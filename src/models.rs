use super::schema::{album, album_invite, folder, media, user};
use chrono::NaiveDateTime;
use rocket::form::FromForm;
use serde::{Serialize, Deserialize};
use rocket_okapi::JsonSchema;

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable)]
#[table_name = "user"]
pub struct User {
  pub id: i32,
  pub username: String,
  pub email: String,
  pub password: String,
}

/// Struct for inserting new users.
#[derive(Insertable, FromForm, Deserialize, JsonSchema, Clone)]
#[table_name = "user"]
pub struct NewUser {
  pub username: String,
  pub email: String,
  pub password: String,
}

impl NewUser {
  pub fn new(username: String, email: String, password: String) -> NewUser {
    return NewUser { username, email, password };
  }
}

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable, Associations, Debug, Clone)]
#[table_name = "folder"]
#[belongs_to(User, foreign_key = "owner_id")]
#[belongs_to(Folder, foreign_key = "parent")]
pub struct Folder {
  pub id: i32,
  pub owner_id: i32,
  pub parent: Option<i32>,
  pub name: String,
}

/// Struct for inserting new folders.
#[derive(Insertable)]
#[table_name = "folder"]
pub struct NewFolder {
  pub owner_id: i32,
  pub parent: Option<i32>,
  pub name: String,
}

impl NewFolder {
  pub fn new(owner_id: i32, name: String, parent: Option<i32>) -> NewFolder {
    return NewFolder { owner_id, name, parent };
  }
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
#[table_name = "media"]
#[belongs_to(Folder, foreign_key = "folder_id")]
#[belongs_to(User, foreign_key = "owner_id")]
pub struct Media {
  pub id: i32,
  pub filename: String,
  pub folder_id: i32,
  pub owner_id: i32,
  pub album_id: Option<i32>,
  pub width: u32,
  pub height: u32,
  pub date_taken: NaiveDateTime,
  pub uuid: String,
  pub sha2_512: String,
}

/// struct for inserting new media
#[derive(Insertable)]
#[table_name = "media"]
pub struct NewMedia {
  pub filename: String,
  pub folder_id: i32,
  pub owner_id: i32,
  pub album_id: Option<i32>,
  pub width: u32,
  pub height: u32,
  pub date_taken: NaiveDateTime,
  pub uuid: String,
  pub sha2_512: String,
}

impl NewMedia {
  pub fn new(filename: String, folder_id: i32, owner_id: i32, album_id: Option<i32>, width: u32, height: u32, date_taken: NaiveDateTime, uuid: String, sha2_512: String) -> NewMedia {
    return NewMedia {
      filename,
      folder_id,
      owner_id,
      album_id,
      width,
      height,
      date_taken,
      uuid,
      sha2_512,
    };
  }
}
