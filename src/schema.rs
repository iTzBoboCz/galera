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
  favourite_photo (id) {
    id -> Integer,
    photo_id -> Integer,
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
  photo (id) {
    id -> Integer,
    filename -> Varchar,
    folder_id -> Integer,
    owner_id -> Integer,
    album_id -> Nullable<Integer>,
    width -> Integer,
    height -> Integer,
    date_taken -> Timestamp,
    sha2_512 -> Varchar,
  }
}

table! {
  user (id) {
    id -> Integer,
    username -> Varchar,
    email -> Varchar,
  }
}

joinable!(album -> user (owner_id));
joinable!(album_invite -> album (album_id));
joinable!(album_invite -> user (invited_user_id));
joinable!(favourite_photo -> photo (photo_id));
joinable!(favourite_photo -> user (user_id));
joinable!(folder -> user (owner_id));
joinable!(photo -> album (album_id));
joinable!(photo -> folder (folder_id));
joinable!(photo -> user (owner_id));

allow_tables_to_appear_in_same_query!(
  album,
  album_invite,
  favourite_photo,
  folder,
  photo,
  user,
);
