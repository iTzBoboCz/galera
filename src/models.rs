use super::schema::{album, album_media, album_invite, auth_access_token, auth_refresh_token, folder, media, favorite_media, user};
use chrono::{Duration, NaiveDateTime, Utc};
use email_address::EmailAddress;
use lazy_regex::regex_is_match;
use nanoid::nanoid;
use rocket_okapi::JsonSchema;
use rocket::form::FromForm;
use serde::{Serialize, Deserialize};
use sha2::Digest;

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
    NewUser { username, email, password }
  }

  /// Encrypts the password.
  // TODO: deduplicate later
  pub fn hash_password(mut self) -> Self {
    let mut hasher = sha2::Sha512::new();
    hasher.update(self.password);
    // {:x} means format as hexadecimal
    self.password = format!("{:X}", hasher.finalize());

    self
  }

  /// Checks the email.
  pub fn is_email_valid(&self) -> bool {
    EmailAddress::is_valid(&self.email)
  }

  /// Runs username, email and password checks.
  pub fn check(&self) -> bool {
    self.check_username() && self.is_email_valid() && self.check_password()
  }

  /// Checks the password.
  ///
  /// # Validity
  ///
  /// The **minimum length is 8 characters** and the **maximum is 128**.\
  /// There are **no limits on what characters you can use**
  /// because it could make cracking passwords easier.\
  /// Maximum length limit is there to prevent long password denial of service
  pub fn check_password(&self) -> bool {
    let len = self.password.chars().count();

    if len < 8 || len > 128 { return false; }

    true
  }

  /// Checks the username.
  ///
  /// # Validity
  ///
  /// The **minimum length is 5 characters** and the **maximum is 30**.\
  /// The first character of a username must be a letter.\
  /// Usernames are low-caps only and can contain these characters:
  /// 1. latin letters (a-z)
  /// 2. numbers (0-9)
  /// 3. underscore (_)
  pub fn check_username(&self) -> bool {
    let len = self.password.chars().count();

    if len < 5 || len > 30 { return false; }

    regex_is_match!(r"^[a-z_][a-z0-9_]{4,29}$", self.username.as_str())
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
    NewFolder { owner_id, name, parent }
  }
}

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable, Associations, Serialize, JsonSchema)]
#[table_name = "album"]
#[belongs_to(User, foreign_key = "owner_id")]
pub struct Album {
  pub id: i32,
  pub owner_id: i32,
  pub name: String,
  pub description: Option<String>,
  pub created_at: NaiveDateTime,
  pub thumbnail_link: Option<String>,
  pub link: String,
  pub password: Option<String>,
}

/// Struct for inserting new albums.
#[derive(Insertable, Deserialize, Clone, JsonSchema)]
#[table_name = "album"]
pub struct NewAlbum {
  pub owner_id: i32,
  pub name: String,
  pub description: Option<String>,
  pub created_at: NaiveDateTime,
  pub link: String,
  pub password: Option<String>,
}

impl NewAlbum {
  pub fn new(owner_id: i32, name: String, description: Option<String>, password: Option<String>) -> NewAlbum {
    let timestamp = Utc::now().timestamp();
    let created_at = NaiveDateTime::from_timestamp(timestamp, 0);
    let link = nanoid!();

    NewAlbum { owner_id, name, description, created_at, link, password }
  }
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

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable, Associations)]
#[table_name = "album_media"]
#[belongs_to(Album, foreign_key = "album_id")]
#[belongs_to(Media, foreign_key = "media_id")]
pub struct AlbumMedia {
  pub id: i32,
  pub album_id: i32,
  pub media_id: i32
}

#[derive(Insertable, Deserialize, JsonSchema)]
#[table_name = "album_media"]
pub struct NewAlbumMedia {
  pub album_id: i32,
  pub media_id: i32
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
  pub width: u32,
  pub height: u32,
  pub date_taken: NaiveDateTime,
  pub uuid: String,
  pub sha2_512: String,
}

impl NewMedia {
  pub fn new(filename: String, folder_id: i32, owner_id: i32, width: u32, height: u32, date_taken: NaiveDateTime, uuid: String, sha2_512: String) -> NewMedia {
    NewMedia {
      filename,
      folder_id,
      owner_id,
      width,
      height,
      date_taken,
      uuid,
      sha2_512,
    }
  }
}

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable, Associations)]
#[table_name = "favorite_media"]
#[belongs_to(Media, foreign_key = "media_id")]
#[belongs_to(User, foreign_key = "user_id")]
pub struct FavoriteMedia {
  pub id: i32,
  pub media_id: i32,
  pub user_id: i32,
}

/// struct for inserting likes.
#[derive(Insertable)]
#[table_name = "favorite_media"]
pub struct NewFavoriteMedia {
  pub media_id: i32,
  pub user_id: i32,
}

impl NewFavoriteMedia {
  pub fn new(media_id: i32,  user_id: i32) -> NewFavoriteMedia {
  NewFavoriteMedia {
      media_id,
      user_id,
    }
  }
}

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable, Associations)]
#[table_name = "auth_refresh_token"]
#[belongs_to(User, foreign_key = "user_id")]
pub struct AuthRefreshToken {
  pub id: i32,
  pub user_id: i32,
  pub refresh_token: String,
  pub expiration_time: NaiveDateTime,
}

/// struct for inserting refresh tokens.
#[derive(Insertable)]
#[table_name = "auth_refresh_token"]
pub struct NewAuthRefreshToken {
  pub user_id: i32,
  pub refresh_token: String,
  pub expiration_time: NaiveDateTime,
}

impl NewAuthRefreshToken {
  pub fn new(user_id: i32, refresh_token: String) -> NewAuthRefreshToken {
    NewAuthRefreshToken {
      user_id,
      refresh_token,
      expiration_time: Utc::now().naive_utc() + Duration::hours(1)
    }
  }
}

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable, Associations)]
#[table_name = "auth_access_token"]
#[belongs_to(AuthRefreshToken, foreign_key = "refresh_token_id")]
pub struct AuthAccessToken {
  pub id: i32,
  pub refresh_token_id: i32,
  pub access_token: String,
  pub expiration_time: NaiveDateTime,
}

/// struct for inserting access tokens.
#[derive(Insertable)]
#[table_name = "auth_access_token"]
pub struct NewAuthAccessToken {
  pub refresh_token_id: i32,
  pub access_token: String,
  pub expiration_time: NaiveDateTime,
}

impl NewAuthAccessToken {
  pub fn new(refresh_token_id: i32, access_token: String) -> NewAuthAccessToken {
    NewAuthAccessToken {
      refresh_token_id,
      access_token,
      expiration_time: Utc::now().naive_utc() + Duration::minutes(15)
    }
  }
}
