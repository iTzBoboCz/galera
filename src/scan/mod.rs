use crate::DbConn;
use crate::db;
use crate::diesel::RunQueryDsl;
use crate::models::{self, *};
use crate::diesel::BoolExpressionMethods;
use crate::diesel::ExpressionMethods;
use crate::diesel::OptionalExtension;
use crate::diesel::QueryDsl;
use crate::schema::folder;
use crate::schema::media;
use chrono::NaiveDateTime;
use db::get_user_username;
use diesel::Table;
use diesel::dsl::sql;
use diesel::sql_query;
use diesel::sql_types::Text;
use diesel::types::Varchar;
use infer;
use std::fs;
use std::fs::create_dir_all;
use std::path::{ Path, PathBuf };
use std::thread::current;
use checksums::{ hash_file, Algorithm::SHA2512 };
use futures::executor;
use uuid::Uuid;

/// checks if the file type is supported.
/// returns **true** for example for **image/jpeg**
/// and **false** for **text/json**
pub fn is_media_supported(pathbuf: &PathBuf) -> bool {
  let valid_mime_types = [
    "image/jpeg",
    "image/png",
    "image/gif",
    "image/webp",
    "image/x-canon-cr2",
    "image/tiff",
    "image/bmp",
    "image/heif",
    "image/avif",
    "video/mp4",
    "video/x-m4v",
    "video/x-matroska",
    "video/webm",
    "video/quicktime",
    "video/x-msvideo",
    "video/x-ms-wmv",
    "video/mpeg",
    "video/x-flv",
    "audio/midi",
    "audio/mpeg",
    "audio/m4a",
    "audio/ogg",
    "audio/x-flac",
    "audio/x-wav",
    "audio/amr",
    "audio/aac",
  ];

  let kind = infer::get_from_path(pathbuf).unwrap();

  if kind.is_none() { return false; }

  if valid_mime_types.contains(&kind.unwrap().mime_type()) {
    trace!("Found: {:?} with type: {:?}", pathbuf, kind.unwrap().mime_type());

    return true;
  }

  return false;
}

/// Scans folders recursively
pub fn scan_recursively(path: PathBuf, array: &mut Vec<PathBuf>) -> bool {
  let mut state = false;

  // skip empty folders
  if path.read_dir().map(|mut i| i.next().is_none()).unwrap_or(false) { return state; }

  let folders = fs::read_dir(path.clone()).unwrap()
    .into_iter()
    .filter(|r| r.is_ok()) // Get rid of Err variants for Result<DirEntry>
    .map(|r| r.unwrap().path()) // This is safe, since we only have the Ok variants
    .filter(|r| r.is_dir())
    .collect::<Vec<PathBuf>>();

  for folder in folders.clone() {
    let found = scan_recursively(folder, array);
    if !state {
      state = found;
    }
  }

  if state {
    return true;
  }

  let files = fs::read_dir(path.clone()).unwrap()
    .into_iter()
    .filter(|r| r.is_ok()) // Get rid of Err variants for Result<DirEntry>
    .map(|r| r.unwrap().path()) // This is safe, since we only have the Ok variants
    .filter(|r| r.is_file())
    .filter(|r| is_media_supported(r))
    .collect::<Vec<PathBuf>>();

  if files.len() > 0 {
    array.push(path);
    return true;
  } else {
    return false;
  }
}

/// scans folder of a given user
pub async fn scan_root(conn: &DbConn, xdg_data: &str, user_id: i32) {
  // root directory
  let username_option = get_user_username(conn, user_id).await;
  if username_option.is_none() { return; }

  let username = username_option.unwrap();

  let current_dir = format!("{}/{}/", xdg_data, username);

  let mut found_folders: Vec<PathBuf> = Vec::new();

  info!("Scanning files and folders for user {} started.", username);

  if !Path::new(&current_dir).exists() {
    let result = create_dir_all(Path::new(&current_dir));

    if result.is_err() {
      error!("Failed to create user folder.");
      return;
    }
  }

  let folders = fs::read_dir(current_dir.clone()).unwrap()
    .into_iter()
    .filter(|r| r.is_ok()) // Get rid of Err variants for Result<DirEntry>
    .map(|r| r.unwrap().path()) // This is safe, since we only have the Ok variants
    .collect::<Vec<PathBuf>>();

  if folders.len() > 0 {
    scan_recursively(PathBuf::from(current_dir), &mut found_folders);
  }

  add_folders_to_db(conn, found_folders, xdg_data, user_id).await;

  scan_folders_for_media(conn, xdg_data, user_id).await;

  info!("Scanning is done.");
}

// folders when using NTFS can be max. 260 characters (we currently support max. 255 - Linux maximum and max. VARCHAR size) TODO: warn user when scanning folder that is longer and skip it
pub async fn add_folders_to_db(conn: &DbConn, paths: Vec<PathBuf>, xdg_data: &str, user_id: i32) {
  // let username_option: Option<String> = conn.run(move |c| async {
  //   return get_user_username(c, user_id).await;
  // }).await;

  let username_option = get_user_username(conn, user_id).await;
  if username_option.is_none() { return; }

  let username = username_option.unwrap();

  let root = format!("{}/{}/", xdg_data, username);

  for path in paths {
    debug!("scanning path: {:?}", path);

    let path_string = path.display().to_string();
    let path_stripped = path_string.strip_prefix(&root).unwrap().to_string().to_owned();
    let string_split = path_stripped.split("/").into_iter().map(|f| f.to_owned()).collect::<Vec<_>>();

    let mut parent: Option<i32> = None;
    let mut i: i32 = 0;
    for s in string_split {
      let folder_id: Option<i32>;
      if i == 0 {
        parent = None;
      }

      folder_id = select_child_folder_id(conn, s.clone(), parent, user_id).await;

      if folder_id.is_none() {
        let new_folder = NewFolder::new(user_id, s.clone(), parent);

        insert_folder(conn, new_folder, s, path.clone()).await;

        let last_insert_id = db::get_last_insert_id(conn).await;

        if last_insert_id.is_none() {
          error!("Last insert id was not returned. This may happen if restarting MySQL during scanning.");
          return;
        }

        parent = Some(last_insert_id.unwrap());
      } else {
        parent = folder_id;
      }

      i = i + 1;
    }
  }
}

pub async fn scan_folders_for_media(conn: &DbConn, xdg_data: &str, user_id: i32) {
  let username_option = get_user_username(conn, user_id).await;
  if username_option.is_none() { return; }

  let username = username_option.unwrap();

  let root_folders = select_root_folders(conn, user_id).await;

  for root_folder in root_folders {
    scan_select(conn, root_folder, String::new(), xdg_data, user_id, username.clone());
  }

  // scan_folder_media - gallery/username
}

pub fn scan_select(conn: &DbConn, parent_folder: Folder, mut path: String, xdg_data: &str, user_id: i32, username: String) {
  if path == "" {
    path = format!("{}/{}/{}/", xdg_data, username, parent_folder.name);
  }
  let folders: Vec<models::Folder> = executor::block_on(select_subfolders(conn, parent_folder.clone(), user_id));

  scan_folder_media(conn, parent_folder.clone(), path.clone(), xdg_data, user_id, username.clone());

  for folder in folders {
    scan_select(conn, folder.clone(), format!("{}/{}/", path.clone(), folder.name), xdg_data, user_id, username.clone());
  }
}

/// Scans user's folder for media
pub fn scan_folder_media(conn: &DbConn, parent_folder: Folder, path: String, xdg_data: &str, user_id: i32, username: String) {
  // get files in a folder
  let media_scanned_option = folder_get_media(PathBuf::from(path.clone()));
  if media_scanned_option.is_none() { return; }

  let media_scanned_vec = media_scanned_option.unwrap();

  if media_scanned_vec.is_empty() { return; }

  let prefix = format!("{}/{}/", xdg_data, username);

  for media_scanned in media_scanned_vec {

    let media_string = media_scanned.display().to_string();
    let name = media_string.strip_prefix(&path).unwrap().to_string().to_owned();

    let media: Option<i32> = executor::block_on(check_if_media_present(conn, name.clone(), parent_folder.clone(), user_id));

    if media.is_none() {
      error!("{:?} doesnt exist in database", media_scanned);

      executor::block_on(insert_media(conn, name, parent_folder.clone(), media_scanned, user_id));
    }
  }
}

pub async fn select_child_folder_id(conn: &DbConn, name: String, parent: Option<i32>, user_id: i32) -> Option<i32> {
  if parent.is_none() {
    conn.run(move |c| {
      folder::table
        .select(folder::id)
        .filter(folder::dsl::parent.is_null().and(folder::dsl::name.eq(name).and(folder::owner_id.eq(user_id))))
        .first::<i32>(c)
        .optional()
        .unwrap()
    }).await

  } else {
    conn.run(move |c| {
      folder::table
        .select(folder::id)
        .filter(folder::dsl::parent.eq(parent).and(folder::dsl::name.eq(name).and(folder::owner_id.eq(user_id))))
        .first::<i32>(c)
        .optional()
        .unwrap()
    }).await
  }
}

pub async fn select_root_folders(conn: &DbConn, user_id: i32) -> Vec<models::Folder> {
  conn.run(move |c| {
    folder::table
      .select(folder::table::all_columns())
      .filter(folder::dsl::parent.is_null().and(folder::owner_id.eq(user_id)))
      .get_results::<Folder>(c)
      .optional()
      .unwrap()
      .unwrap()
  }).await
}

pub async fn select_subfolders(conn: &DbConn, parent_folder: Folder, user_id: i32) -> Vec<models::Folder> {
  conn.run(move |c| {
    folder::table
      .select(folder::table::all_columns())
      .filter(folder::dsl::parent.eq(parent_folder.id).and(folder::owner_id.eq(user_id)))
      .get_results::<Folder>(c)
      .optional()
      .unwrap()
      .unwrap()
  }).await
}

/// Selects folder from folder id.
/// # Example
/// We're selecting folder with id 10.
/// ```
/// let folder: Folder = select_folder(&conn, 10);
/// ```
pub async fn select_folder(conn: &DbConn, folder_id: i32) -> Option<models::Folder> {
  conn.run(move |c| {
    folder::table
      .select(folder::table::all_columns())
      .filter(folder::dsl::id.eq(folder_id))
      .first::<Folder>(c)
      .optional()
      .unwrap()
  }).await
}

/// Recursively selects parent folder.\
/// You need to pass a vector to which the folders will be appended.
/// # Example
/// We're selecting all parent folders of a folder with id 10, where user id is 1.
/// ```
/// let mut folders: Vec<Folder> = vec!();
/// let current_folder = Folder { id: 15, owner_id: 1, parent: Some(10), name: "some_folder" }
/// folders.push(current_folder.clone());
///
/// scan::select_parent_folder_recursive(&conn, current_folder, user_id, &mut folders);
///
/// // This produces:
/// // folders: [Folder { id: 15, owner_id: 1, parent: Some(10), name: "some_folder" }, Folder { id: 10, owner_id: 1, parent: None, name: "root_folder" }]
/// ```
// TODO: Write faster recursive function with diesel's sql_query()
pub fn select_parent_folder_recursive(conn: &DbConn, current_folder: Folder, user_id: i32, vec: &mut Vec<Folder>) -> bool {
  let parent = executor::block_on(select_parent_folder(conn, current_folder, user_id));
  if parent.is_none() { return false; }

  vec.push(parent.clone().unwrap());

  return select_parent_folder_recursive(conn, parent.clone().unwrap(), user_id, vec);
}

/// Selects parent folder.
/// # Example
/// We're selecting parent folder of a folder with id 10, where user id is 1.
/// ```
/// let current_folder: Folder = select_folder(&conn, 10);
/// let parent_folder: Option<Folder> = select_parent_folder(&conn, current_folder, 1);
/// ```
pub async fn select_parent_folder(conn: &DbConn, current_folder: Folder, user_id: i32) -> Option<Folder> {
  if current_folder.parent.is_none() { return None; }
  conn.run(move |c| {
    folder::table
      .select(folder::table::all_columns())
      .filter(folder::dsl::id.eq(current_folder.parent.unwrap()).and(folder::owner_id.eq(user_id)))
      .first::<Folder>(c)
      .ok()
  }).await
}

pub async fn check_if_media_present(conn: &DbConn, name: String, parent_folder: Folder, user_id: i32) -> Option<i32> {
  conn.run(move |c| {
    // check wheter the file is already in a database
    return media::table
      .select(media::id)
      .filter(media::dsl::filename.eq(name).and(media::owner_id.eq(user_id).and(media::folder_id.eq(parent_folder.id))))
      .first::<i32>(c)
      .optional()
      .unwrap();
  }).await
}

pub async fn insert_media(conn: &DbConn, name: String, parent_folder: Folder, media_scanned:PathBuf, user_id: i32) {
  conn.run(move |c| {
    // error!("file {} doesnt exist", name.display().to_string());
    let uuid = Uuid::new_v4().to_string();
    let new_media = NewMedia::new(name.clone(), parent_folder.id, user_id, None, 0, 0, NaiveDateTime::from_timestamp(10, 10), uuid, hash_file(&media_scanned, SHA2512));
    let insert = diesel::insert_into(media::table)
      .values(new_media)
      .execute(c)
      .expect(format!("Error inserting file {:?}", name).as_str());

    return insert;
  }).await;
}

pub async fn insert_folder(conn: &DbConn, new_folder: NewFolder, name: String, path: PathBuf) {
  conn.run(move |c| {
    let insert = diesel::insert_into(folder::table)
      .values(new_folder)
      .execute(c)
      .expect(format!("Error scanning folder {} in {}", name, path.display().to_string()).as_str());

    return insert;
  }).await;
}

pub fn folder_get_media(dir: PathBuf) -> Option<Vec<PathBuf>> {
  if !dir.exists() { return None; }

  let data: Vec<PathBuf> = fs::read_dir(&dir).unwrap()
    .into_iter()
    .filter(|r| r.is_ok()) // Get rid of Err variants for Result<DirEntry>
    .map(|r| r.unwrap().path()) // This is safe, since we only have the Ok variants
    .filter(|r| r.is_file()) // Get rid of Err variants for Result<DirEntry>
    .filter(|r| is_media_supported(r)) // Filter out non-folders
    .collect();

  return Some(data);
}
