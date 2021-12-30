table! {
  album (id) {
    id -> Integer,
    owner_id -> Integer,
    name -> Varchar,
    description -> Nullable<Varchar>,
    created_at -> Timestamp,
    thumbnail_link -> Nullable<Varchar>,
    link -> Varchar,
    password -> Nullable<Varchar>,
  }
}

table! {
  album_invite (id) {
    id -> Integer,
    album_id -> Integer,
    invited_user_id -> Integer,
    accepted -> Bool,
    write_access -> Bool,
  }
}

table! {
  album_media (id) {
    id -> Integer,
    album_id -> Integer,
    media_id -> Integer,
  }
}

table! {
  auth_access_token (id) {
    id -> Integer,
    refresh_token_id -> Integer,
    access_token -> Varchar,
    expiration_time -> Timestamp,
  }
}

table! {
  auth_refresh_token (id) {
    id -> Integer,
    user_id -> Integer,
    refresh_token -> Varchar,
    expiration_time -> Timestamp,
  }
}

table! {
  favorite_media (id) {
    id -> Integer,
    media_id -> Integer,
    user_id -> Integer,
  }
}

table! {
  folder (id) {
    id -> Integer,
    owner_id -> Integer,
    parent -> Nullable<Integer>,
    name -> Varchar,
  }
}

table! {
  media (id) {
    id -> Integer,
    filename -> Varchar,
    folder_id -> Integer,
    owner_id -> Integer,
    width -> Unsigned<Integer>,
    height -> Unsigned<Integer>,
    description -> Nullable<Varchar>,
    date_taken -> Timestamp,
    uuid -> Varchar,
    sha2_512 -> Varchar,
  }
}

table! {
  user (id) {
    id -> Integer,
    username -> Varchar,
    email -> Varchar,
    password -> Varchar,
  }
}

joinable!(album -> user (owner_id));
joinable!(album_invite -> album (album_id));
joinable!(album_invite -> user (invited_user_id));
joinable!(album_media -> album (album_id));
joinable!(album_media -> media (media_id));
joinable!(auth_access_token -> auth_refresh_token (refresh_token_id));
joinable!(auth_refresh_token -> user (user_id));
joinable!(favorite_media -> media (media_id));
joinable!(favorite_media -> user (user_id));
joinable!(folder -> user (owner_id));
joinable!(media -> folder (folder_id));
joinable!(media -> user (owner_id));

allow_tables_to_appear_in_same_query!(
  album,
  album_invite,
  album_media,
  auth_access_token,
  auth_refresh_token,
  favorite_media,
  folder,
  media,
  user,
);
