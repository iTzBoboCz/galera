use super::schema::{
  album, album_media, album_invite, album_share_link, auth_access_token, auth_refresh_token,
  folder, media, favorite_media, user,
};
use chrono::{Duration, NaiveDateTime, Utc};
use email_address::EmailAddress;
use lazy_regex::regex_is_match;
use nanoid::nanoid;
// use rocket_okapi::JsonSchema;
// use rocket::form::FromForm;
use serde::{Serialize, Deserialize};
use sha2::Digest;
use uuid::Uuid;

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable)]
#[diesel(table_name = user)]
pub struct User {
  pub id: i32,
  pub uuid: String,
  pub username: String,
  pub email: String,
  pub password: String,
}

/// Struct for receiving new users.
// #[derive(FromForm, JsonSchema)]
#[derive(Insertable, Deserialize, Clone)]
#[diesel(table_name = user)]
pub struct NewUser {
  pub username: String,
  pub email: String,
  pub password: String,
}

impl NewUser {
  pub fn new(username: String, email: String, password: String) -> NewUser {
    NewUser {
      username,
      email,
      password,
    }
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

    if len < 8 || len > 128 {
      return false;
    }

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

    if len < 5 || len > 30 {
      return false;
    }

    regex_is_match!(r"^[a-z_][a-z0-9_]{4,29}$", self.username.as_str())
  }
}

/// Struct for inserting new users.
// #[derive(FromForm, JsonSchema)]
#[derive(Insertable, Deserialize, Clone)]
#[diesel(table_name = user)]
pub struct InsertUser {
  pub uuid: String,
  pub username: String,
  pub email: String,
  pub password: String,
}

impl From<NewUser> for InsertUser {
  fn from(new_user: NewUser) -> Self {
    InsertUser { uuid: Uuid::new_v4().to_string(), username: new_user.username, email: new_user.email, password: new_user.password }
  }
}

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable, Associations, Debug, Clone)]
#[diesel(table_name = folder)]
#[diesel(belongs_to(User, foreign_key = owner_id))]
#[diesel(belongs_to(Folder, foreign_key = parent))]
pub struct Folder {
  pub id: i32,
  pub uuid: String,
  pub owner_id: i32,
  pub parent: Option<i32>,
  pub name: String,
}

/// Struct for inserting new folders.
#[derive(Insertable, Clone)]
#[diesel(table_name = folder)]
pub struct NewFolder {
  pub uuid: String,
  pub owner_id: i32,
  pub parent: Option<i32>,
  pub name: String,
}

impl NewFolder {
  pub fn new(owner_id: i32, name: String, parent: Option<i32>) -> NewFolder {
    NewFolder {
      uuid: Uuid::new_v4().to_string(),
      owner_id,
      name,
      parent,
    }
  }
}

#[allow(non_camel_case_types)]
// #[derive(JsonSchema)]
#[derive(Identifiable, Queryable, Associations, Serialize)]
#[diesel(table_name = album)]
#[diesel(belongs_to(User, foreign_key = owner_id))]
pub struct Album {
  pub id: i32,
  pub uuid: String,
  pub owner_id: i32,
  pub name: String,
  pub description: Option<String>,
  pub created_at: NaiveDateTime,
  pub thumbnail_link: Option<String>,
  pub password: Option<String>,
}

/// Struct for inserting new albums.
// #[derive(JsonSchema)]
#[derive(Insertable, Deserialize, Clone)]
#[diesel(table_name = album)]
pub struct NewAlbum {
  pub uuid: String,
  pub owner_id: i32,
  pub name: String,
  pub description: Option<String>,
  pub created_at: NaiveDateTime,
  pub password: Option<String>,
}

impl NewAlbum {
  pub fn new(
    owner_id: i32,
    name: String,
    description: Option<String>,
    password: Option<String>,
  ) -> NewAlbum {
    let timestamp = Utc::now().timestamp();
    let created_at = NaiveDateTime::from_timestamp(timestamp, 0);

    NewAlbum {
      uuid: Uuid::new_v4().to_string(),
      owner_id,
      name,
      description,
      created_at,
      password,
    }
  }
}

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable, Associations)]
#[diesel(table_name = album_invite)]
#[diesel(belongs_to(Album, foreign_key = album_id))]
#[diesel(belongs_to(User, foreign_key = invited_user_id))]
pub struct Album_invite {
  pub id: i32,
  pub uuid: String,
  pub album_id: i32,
  pub invited_user_id: i32,
  pub accepted: bool,
  pub write_access: bool,
}

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable, Associations)]
#[diesel(table_name = album_share_link)]
#[diesel(belongs_to(Album, foreign_key = album_id))]
pub struct AlbumShareLink {
  pub id: i32,
  pub uuid: String,
  pub album_id: i32,
  pub link: String,
  pub password: Option<String>,
  pub expiration: Option<NaiveDateTime>,
}

#[allow(non_camel_case_types)]
#[derive(Insertable, Clone)]
#[diesel(table_name = album_share_link)]
pub struct NewAlbumShareLink {
  pub uuid: String,
  pub album_id: i32,
  pub link: String,
  pub password: Option<String>,
  pub expiration: Option<NaiveDateTime>,
}

impl NewAlbumShareLink {
  pub fn new(album_id: i32, password: Option<String>, expiration: Option<NaiveDateTime>) -> Self {
    let uuid = Uuid::new_v4().to_string();
    let link = nanoid!();

    Self {
      uuid,
      album_id,
      link,
      password,
      expiration,
    }
  }
}

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable, Associations)]
#[diesel(table_name = album_media)]
#[diesel(belongs_to(Album, foreign_key = album_id))]
#[diesel(belongs_to(Media, foreign_key = media_id))]
pub struct AlbumMedia {
  pub id: i32,
  pub uuid: String,
  pub album_id: i32,
  pub media_id: i32,
}

// #[derive(JsonSchema)]
#[derive(Insertable, Deserialize)]
#[diesel(table_name = album_media)]
pub struct NewAlbumMedia {
  pub uuid: String,
  pub album_id: i32,
  pub media_id: i32,
}

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable, Associations)]
#[diesel(table_name = media)]
#[diesel(belongs_to(Folder, foreign_key = folder_id))]
#[diesel(belongs_to(User, foreign_key = owner_id))]
pub struct Media {
  pub id: i32,
  pub uuid: String,
  pub filename: String,
  pub folder_id: i32,
  pub owner_id: i32,
  pub width: u32,
  pub height: u32,
  pub description: Option<String>,
  pub date_taken: NaiveDateTime,
  pub sha2_512: String,
}

/// struct for inserting new media
#[derive(Insertable)]
#[diesel(table_name = media)]
pub struct NewMedia {
  pub uuid: String,
  pub filename: String,
  pub folder_id: i32,
  pub owner_id: i32,
  pub width: u32,
  pub height: u32,
  pub description: Option<String>,
  pub date_taken: NaiveDateTime,
  pub sha2_512: String,
}

impl NewMedia {
  pub fn new(
    filename: String,
    folder_id: i32,
    owner_id: i32,
    width: u32,
    height: u32,
    description: Option<String>,
    date_taken: NaiveDateTime,
    sha2_512: String,
  ) -> NewMedia {
    let uuid = Uuid::new_v4().to_string();

    NewMedia {
      uuid,
      filename,
      folder_id,
      owner_id,
      width,
      height,
      description,
      date_taken,
      sha2_512,
    }
  }
}

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable, Associations)]
#[diesel(table_name = favorite_media)]
#[diesel(belongs_to(Media, foreign_key = media_id))]
#[diesel(belongs_to(User, foreign_key = user_id))]
pub struct FavoriteMedia {
  pub id: i32,
  pub uuid: String,
  pub media_id: i32,
  pub user_id: i32,
}

/// struct for inserting likes.
#[derive(Insertable)]
#[diesel(table_name = favorite_media)]
pub struct NewFavoriteMedia {
  pub uuid: String,
  pub media_id: i32,
  pub user_id: i32,
}

impl NewFavoriteMedia {
  pub fn new(media_id: i32, user_id: i32) -> NewFavoriteMedia {
    let uuid = Uuid::new_v4().to_string();

    NewFavoriteMedia { uuid, media_id, user_id }
  }
}

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable, Associations)]
#[diesel(table_name = auth_refresh_token)]
#[diesel(belongs_to(User, foreign_key = user_id))]
pub struct AuthRefreshToken {
  pub id: i32,
  pub uuid: String,
  pub user_id: i32,
  pub refresh_token: String,
  pub expiration_time: NaiveDateTime,
}

/// struct for inserting refresh tokens.
#[derive(Insertable)]
#[diesel(table_name = auth_refresh_token)]
pub struct NewAuthRefreshToken {
  pub uuid: String,
  pub user_id: i32,
  pub refresh_token: String,
  pub expiration_time: NaiveDateTime,
}

impl NewAuthRefreshToken {
  pub fn new(user_id: i32, refresh_token: String) -> NewAuthRefreshToken {
    NewAuthRefreshToken {
      uuid: Uuid::new_v4().to_string(),
      user_id,
      refresh_token,
      expiration_time: Utc::now().naive_utc() + Duration::hours(1),
    }
  }
}

#[allow(non_camel_case_types)]
#[derive(Identifiable, Queryable, Associations)]
#[diesel(table_name = auth_access_token)]
#[diesel(belongs_to(AuthRefreshToken, foreign_key = refresh_token_id))]
pub struct AuthAccessToken {
  pub id: i32,
  pub uuid: String,
  pub refresh_token_id: i32,
  pub access_token: String,
  pub expiration_time: NaiveDateTime,
}

/// struct for inserting access tokens.
#[derive(Insertable, Clone)]
#[diesel(table_name = auth_access_token)]
pub struct NewAuthAccessToken {
  pub uuid: String,
  pub refresh_token_id: i32,
  pub access_token: String,
  pub expiration_time: NaiveDateTime,
}

impl NewAuthAccessToken {
  pub fn new(refresh_token_id: i32, access_token: String) -> NewAuthAccessToken {
    NewAuthAccessToken {
      uuid: Uuid::new_v4().to_string(),
      refresh_token_id,
      access_token,
      expiration_time: Utc::now().naive_utc() + Duration::minutes(15),
    }
  }
}
