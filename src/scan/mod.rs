use crate::Pool;
use crate::db;
use crate::diesel::RunQueryDsl;
use crate::models::*;
use crate::diesel::BoolExpressionMethods;
use crate::diesel::ExpressionMethods;
use crate::diesel::OptionalExtension;
use crate::diesel::QueryDsl;
use crate::schema::folder;
use crate::schema::media;
use chrono::NaiveDateTime;
use db::get_user_username;
use diesel::Table;
use infer;
use std::fs;
use std::path::PathBuf;
use checksums::{ hash_file, Algorithm::SHA2512 };

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
pub fn scan_root(pool: Pool, xdg_data: &str, user_id: i32) {
  // root directory
  let username_option = get_user_username(pool.clone(), user_id);
  if username_option.is_none() { return; }

  let username = username_option.unwrap();

  let current_dir = format!("{}/{}/", xdg_data, username);

  let mut found_folders: Vec<PathBuf> = Vec::new();

  info!("Scanning files and folders for user {} started.", username);

  let folders = fs::read_dir(current_dir.clone()).unwrap()
    .into_iter()
    .filter(|r| r.is_ok()) // Get rid of Err variants for Result<DirEntry>
    .map(|r| r.unwrap().path()) // This is safe, since we only have the Ok variants
    .collect::<Vec<PathBuf>>();

  if folders.len() > 0 {
    scan_recursively(PathBuf::from(current_dir), &mut found_folders);
  }

  add_folders_to_db(pool.clone(), found_folders, xdg_data, user_id);

  scan_folders_for_media(pool, xdg_data, user_id);

  info!("Scanning is done.");
}

// folders when using NTFS can be max. 260 characters (we currently support max. 255 - Linux maximum and max. VARCHAR size) TODO: warn user when scanning folder that is longer and skip it
pub fn add_folders_to_db(pool: Pool, paths: Vec<PathBuf>, xdg_data: &str, user_id: i32) {
  let username_option = get_user_username(pool.clone(), user_id);
  if username_option.is_none() { return; }

  let username = username_option.unwrap();

  let root = format!("{}/{}/", xdg_data, username);

  for path in paths {
    debug!("scanning path: {:?}", path);

    let path_string = path.display().to_string();
    let path_stripped = path_string.strip_prefix(&root).unwrap();
    let string_split = path_stripped.split("/").collect::<Vec<_>>();

    let mut parent: Option<i32> = None;
    let mut i = 0;
    for s in string_split {
      let folder_id: Option<i32>;
      if i == 0 {
        parent = None;

        folder_id = folder::table
          .select(folder::id)
          .filter(folder::dsl::parent.is_null().and(folder::dsl::name.eq(s).and(folder::owner_id.eq(user_id))))
          .first::<i32>(&pool.clone().get().unwrap())
          .optional()
          .unwrap();
      } else {
        folder_id = folder::table
          .select(folder::id)
          .filter(folder::dsl::parent.eq(parent).and(folder::dsl::name.eq(s).and(folder::owner_id.eq(user_id))))
          .first::<i32>(&pool.clone().get().unwrap())
          .optional()
          .unwrap();
      }

      if folder_id.is_none() {
        let new_folder = NewFolder::new(user_id, String::from(s), parent);

        diesel::insert_into(folder::table)
          .values(new_folder)
          .execute(&pool.get().unwrap())
          .expect(format!("Error scanning folder {} in {}", s, path_string).as_str());

          let last_insert_id = db::get_last_insert_id(pool.clone());

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

pub fn scan_folders_for_media(pool: Pool, xdg_data: &str, user_id: i32) {
  let username_option = get_user_username(pool.clone(), user_id);
  if username_option.is_none() { return; }

  let username = username_option.unwrap();

  let root_folders = folder::table
    .select(folder::table::all_columns())
    .filter(folder::dsl::parent.is_null().and(folder::owner_id.eq(user_id)))
    .get_results::<Folder>(&pool.clone().get().unwrap())
    .optional()
    .unwrap()
    .unwrap();

  for root_folder in root_folders {
    scan_select(pool.clone(), root_folder, String::new(), xdg_data, user_id, username.clone());
  }

  // scan_folder_media - gallery/username
}

pub fn scan_select(pool: Pool, parent_folder: Folder, mut path: String, xdg_data: &str, user_id: i32, username: String) {
  if path == "" {
    path = format!("{}/{}/{}", xdg_data, username, parent_folder.name);
  }
  let folders = folder::table
    .select(folder::table::all_columns())
    .filter(folder::dsl::parent.eq(parent_folder.id).and(folder::owner_id.eq(user_id)))
    .get_results::<Folder>(&pool.clone().get().unwrap())
    .optional()
    .unwrap()
    .unwrap();

  scan_folder_media(pool.clone(), parent_folder.clone(), path.clone(), xdg_data, user_id, username.clone());

  for folder in folders {
    scan_select(pool.clone(), folder.clone(), format!("{}/{}", path.clone(), folder.name), xdg_data, user_id, username.clone());
  }
}

/// Scans user's folder for media
pub fn scan_folder_media(pool: Pool, parent_folder: Folder, path: String, xdg_data: &str, user_id: i32, username: String) {
  // get files in a folder
  let media_scanned_option = folder_get_media(PathBuf::from(path.clone()));
  if media_scanned_option.is_none() { return; }

  let media_scanned_vec = media_scanned_option.unwrap();

  if media_scanned_vec.is_empty() { return; }

  let prefix = format!("{}/{}", xdg_data, username);

  for media_scanned in media_scanned_vec {
    let name = media_scanned.strip_prefix(&path).unwrap();

    // check wheter the file is already in a database
    let media = media::table
      .select(media::id)
      .filter(media::dsl::filename.eq(name.display().to_string()).and(media::owner_id.eq(user_id).and(media::folder_id.eq(parent_folder.id))))
      .first::<i32>(&pool.clone().get().unwrap())
      .optional()
      .unwrap();

    if media.is_none() {
      error!("{:?} doesnt exist in database", media_scanned);

      // error!("file {} doesnt exist", name.display().to_string());
      let new_media = NewMedia::new(name.display().to_string(), parent_folder.id, user_id, None, 0, 0, NaiveDateTime::from_timestamp(10, 10), hash_file(&media_scanned, SHA2512));
      diesel::insert_into(media::table)
        .values(new_media)
        .execute(&pool.get().unwrap())
        .expect(format!("Error inserting file {:?}", media).as_str());
    }
  }
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
