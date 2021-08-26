table! {
  album (id) {
    id -> Integer,
    owner_id -> Integer,
    link -> Nullable<Varchar>,
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
    album_id -> Nullable<Integer>,
    width -> Unsigned<Integer>,
    height -> Unsigned<Integer>,
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
joinable!(favorite_media -> media (media_id));
joinable!(favorite_media -> user (user_id));
joinable!(folder -> user (owner_id));
joinable!(media -> album (album_id));
joinable!(media -> folder (folder_id));
joinable!(media -> user (owner_id));

allow_tables_to_appear_in_same_query!(
  album,
  album_invite,
  favorite_media,
  folder,
  media,
  user,
);
