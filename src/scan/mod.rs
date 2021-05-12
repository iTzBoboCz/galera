use crate::Pool;
use crate::db;
use crate::diesel::RunQueryDsl;
use crate::models::*;
use crate::diesel::BoolExpressionMethods;
use crate::diesel::ExpressionMethods;
use crate::diesel::OptionalExtension;
use crate::diesel::QueryDsl;
use crate::schema::folder;
use db::get_user_username;
use infer;
use std::fs;
use std::path::PathBuf;

pub fn is_media_suppoted(pathbuf: &PathBuf) -> bool {
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
    "application/json"
  ];

  let kind = infer::get_from_path(pathbuf).unwrap();

  if kind.is_none() { return false };

  if valid_mime_types.contains(&kind.unwrap().mime_type()) {
    info!("Found: {:?} with type: {:?}", pathbuf, kind.unwrap().mime_type());

    return true;
  }

  return false;
}

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
    .filter(|r| is_media_suppoted(r))
    .collect::<Vec<PathBuf>>();

  if files.len() > 0 {
    array.push(path);
    return true;
  } else {
    return false;
  }
}

pub fn scan_root(pool: Pool, xdg_data: &str, user_id: i32) {
  // root directory
  let username = get_user_username(pool.clone(), user_id);
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

  add_folders_to_db(pool, found_folders, xdg_data, user_id);

  info!("Scanning is done.");
}

// folders when using NTFS can be max. 260 characters (we currently support max. 255 - Linux maximum and max. VARCHAR size) TODO: warn user when scanning folder that is longer and skip it
pub fn add_folders_to_db(pool: Pool, paths: Vec<PathBuf>, xdg_data: &str, user_id: i32) {
  use crate::schema::folder;

  let username = get_user_username(pool.clone(), user_id);
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
        let new_folder = NewFolder::new(user_id, String::from(s), parent );

        diesel::insert_into(folder::table)
          .values(new_folder)
          .execute(&pool.get().unwrap())
          .expect(format!("Error scanning folder {} in {}", s, path_string).as_str());

        parent = Some(db::get_last_insert_id(pool.clone()));
      } else {
        parent = folder_id;
      }

      i = i + 1;
    }
  }
}
