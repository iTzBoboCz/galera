// @generated automatically by Diesel CLI.

#![allow(unused_qualifications)]

diesel::table! {
  album (id) {
    id -> Integer,
    owner_id -> Integer,
    #[max_length = 255]
    name -> Varchar,
    #[max_length = 255]
    description -> Nullable<Varchar>,
    created_at -> Timestamp,
    #[max_length = 36]
    thumbnail_link -> Nullable<Varchar>,
    #[max_length = 21]
    link -> Varchar,
    #[max_length = 255]
    password -> Nullable<Varchar>,
  }
}

diesel::table! {
  album_invite (id) {
    id -> Integer,
    album_id -> Integer,
    invited_user_id -> Integer,
    accepted -> Bool,
    write_access -> Bool,
  }
}

diesel::table! {
  album_media (id) {
    id -> Integer,
    album_id -> Integer,
    media_id -> Integer,
  }
}

diesel::table! {
  album_share_link (id) {
    id -> Integer,
    #[max_length = 36]
    uuid -> Char,
    album_id -> Integer,
    #[max_length = 21]
    link -> Varchar,
    #[max_length = 128]
    password -> Nullable<Varchar>,
    expiration -> Nullable<Datetime>,
  }
}

diesel::table! {
  auth_access_token (id) {
    id -> Integer,
    refresh_token_id -> Integer,
    #[max_length = 255]
    access_token -> Varchar,
    expiration_time -> Timestamp,
  }
}

diesel::table! {
  auth_refresh_token (id) {
    id -> Integer,
    user_id -> Integer,
    #[max_length = 255]
    refresh_token -> Varchar,
    expiration_time -> Timestamp,
  }
}

diesel::table! {
  favorite_media (id) {
    id -> Integer,
    media_id -> Integer,
    user_id -> Integer,
  }
}

diesel::table! {
  folder (id) {
    id -> Integer,
    owner_id -> Integer,
    parent -> Nullable<Integer>,
    #[max_length = 255]
    name -> Varchar,
  }
}

diesel::table! {
  media (id) {
    id -> Integer,
    #[max_length = 255]
    filename -> Varchar,
    folder_id -> Integer,
    owner_id -> Integer,
    width -> Unsigned<Integer>,
    height -> Unsigned<Integer>,
    #[max_length = 255]
    description -> Nullable<Varchar>,
    date_taken -> Timestamp,
    #[max_length = 36]
    uuid -> Varchar,
    #[max_length = 128]
    sha2_512 -> Varchar,
  }
}

diesel::table! {
  user (id) {
    id -> Integer,
    #[max_length = 60]
    username -> Varchar,
    #[max_length = 254]
    email -> Varchar,
    #[max_length = 128]
    password -> Varchar,
  }
}

diesel::joinable!(album -> user (owner_id));
diesel::joinable!(album_invite -> album (album_id));
diesel::joinable!(album_invite -> user (invited_user_id));
diesel::joinable!(album_media -> album (album_id));
diesel::joinable!(album_media -> media (media_id));
diesel::joinable!(album_share_link -> album (album_id));
diesel::joinable!(auth_access_token -> auth_refresh_token (refresh_token_id));
diesel::joinable!(auth_refresh_token -> user (user_id));
diesel::joinable!(favorite_media -> media (media_id));
diesel::joinable!(favorite_media -> user (user_id));
diesel::joinable!(folder -> user (owner_id));
diesel::joinable!(media -> folder (folder_id));
diesel::joinable!(media -> user (owner_id));

diesel::allow_tables_to_appear_in_same_query!(
  album,
  album_invite,
  album_media,
  album_share_link,
  auth_access_token,
  auth_refresh_token,
  favorite_media,
  folder,
  media,
  user,
);
