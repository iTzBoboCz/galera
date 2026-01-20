use crate::{db, ConnectionPool};
use crate::models::{Folder, NewFolder};

use futures::executor;
use tracing::{error, info, warn, trace, debug};
use std::fs;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};

/// checks if the file type is supported.
/// returns **true** for example for **image/jpeg**
/// and **false** for **text/json**
pub fn is_media_supported(pathbuf: &Path) -> bool {
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

  false
}

#[allow(dead_code)]
pub struct Scan {
  user_id: i32,
  username: String,
  directory: PathBuf
}

impl Scan {
  pub async fn new(pool: ConnectionPool, user_id: i32, directory: PathBuf) -> Option<Self> {
    let username = db::users::get_user_username(pool.get().await.unwrap(), user_id).await?;

    Some(Self {
      user_id,
      username,
      directory
    })
  }

  /// Outputs a list of populated folders.
  // TODO: find out which is better: strip -> sort vs sort -> strip
  pub fn get_folders(&self) -> Vec<PathBuf> {
    let mut dirs = vec![];

    for entry in walkdir::WalkDir::new(PathBuf::from(&self.directory).join(&self.username)) {
      if entry.is_ok() {
        let path = entry.unwrap().into_path();
        if path.is_file() {
          if let Some(parent) = path.parent() {
            let strip = PathBuf::from(parent.strip_prefix(&self.directory).unwrap());
            dirs.push(strip);
          }
        }
      }
    }

    dirs.sort();
    dirs.dedup();

    dirs
  }
}

/// scans folder of a given user
pub async fn scan_root(pool: ConnectionPool, xdg_data: PathBuf, user_id: i32) {
  // root directory
  let username_option = db::users::get_user_username(pool.get().await.unwrap(), user_id).await;
  if username_option.is_none() { return; }

  let username = username_option.unwrap();

  let current_dir = xdg_data.join(username.clone());

  info!("Scanning files and folders for user {} started.", username);

  if !Path::new(&current_dir).exists() {
    let result = create_dir_all(Path::new(&current_dir));

    if result.is_err() {
      error!("Failed to create user folder.");
      return;
    }
  }

  let scan = Scan::new(pool.clone(), user_id, xdg_data.clone()).await;
  if scan.is_none() { return }

  let found_folders = scan.unwrap().get_folders();

  add_folders_to_db(pool.clone(), found_folders, user_id).await;

  scan_folders_for_media(pool, xdg_data, user_id).await;

  info!("Scanning is done.");
}

// folders when using NTFS can be max. 260 characters (we currently support max. 255 - Linux maximum and max. VARCHAR size) TODO: warn user when scanning folder that is longer and skip it
pub async fn add_folders_to_db(pool: ConnectionPool, relative_paths: Vec<PathBuf>, user_id: i32) {
  for path in relative_paths {
    debug!("scanning path: {:?}", path);

    let string_split = path.to_str().unwrap().split('/').into_iter().map(|f| f.to_owned());

    let mut parent: Option<i32> = None;
    for (i, s) in string_split.enumerate() {
      let folder_id: Option<i32>;
      if i == 0 {
        parent = None;
      }

      folder_id = db::folders::select_child_folder_id(pool.get().await.unwrap(), s.clone(), parent, user_id).await;

      if folder_id.is_none() {
        let new_folder = NewFolder::new(user_id, s.clone(), parent);

        db::folders::insert_folder(pool.get().await.unwrap(), new_folder, s, path.clone()).await;

        let last_insert_id = db::general::get_last_insert_id(pool.get().await.unwrap()).await;

        if last_insert_id.is_none() {
          error!("Last insert id was not returned. This may happen if restarting MySQL during scanning.");
          return;
        }

        parent = Some(last_insert_id.unwrap());
      } else {
        parent = folder_id;
      }
    }
  }
}

pub async fn scan_folders_for_media(pool: ConnectionPool, xdg_data: PathBuf, user_id: i32) {
  let username_option = db::users::get_user_username(pool.get().await.unwrap(), user_id).await;
  if username_option.is_none() { return; }

  let username = username_option.unwrap();

  let root_folder_result = db::folders::select_root_folder(pool.get().await.unwrap(), user_id).await;
  if root_folder_result.is_err() { return }

  let root_folder_option = root_folder_result.unwrap();
  if root_folder_option.is_none() { return }

  scan_select(pool, root_folder_option.unwrap(), None, xdg_data, user_id, username.clone());
}

pub fn scan_select(pool: ConnectionPool, parent_folder: Folder, mut path: Option<PathBuf>, xdg_data: PathBuf, user_id: i32, username: String) {
  if path.is_none() {
    path = Some(xdg_data.join(parent_folder.name.clone()));
  }
  let folders: Vec<Folder> = executor::block_on(db::folders::select_subfolders(executor::block_on(pool.get()).unwrap(), parent_folder.clone(), user_id));

  let path_clean = path.unwrap();

  scan_folder_media(pool.clone(), parent_folder, path_clean.clone(), user_id);

  for folder in folders {
    scan_select(pool.clone(), folder.clone(), Some(path_clean.clone().join(folder.name)), xdg_data.clone(), user_id, username.clone());
  }
}

/// Scans user's folder for media
pub fn scan_folder_media(pool: ConnectionPool, parent_folder: Folder, path: PathBuf, user_id: i32) {
  // get files in a folder
  let media_scanned_option = folder_get_media(path);
  if media_scanned_option.is_none() { return; }

  let media_scanned_vec = media_scanned_option.unwrap();

  if media_scanned_vec.is_empty() { return; }

  for media_scanned in media_scanned_vec {
    let name = media_scanned.file_name().unwrap().to_str().unwrap().to_owned();

    let media: Option<i32> = executor::block_on(db::media::check_if_media_present(executor::block_on(pool.get()).unwrap(), name.clone(), parent_folder.clone(), user_id));

    if media.is_none() {
      debug!("{:?} doesnt exist in database", media_scanned);

      let image_dimensions = image::image_dimensions(media_scanned.clone())
        .ok();

      if image_dimensions.is_none() {
        warn!("Image {:?} was skipped as its dimensions are unknown.", media_scanned);
        continue;
      }

      executor::block_on(db::media::insert_media(executor::block_on(pool.get()).unwrap(), name, parent_folder.clone(), user_id,  image_dimensions.unwrap(), None, media_scanned));
    }
  }
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
pub fn select_parent_folder_recursive(pool: ConnectionPool, current_folder: Folder, user_id: i32, vec: &mut Vec<Folder>) -> bool {
  let parent = executor::block_on(db::folders::select_parent_folder(executor::block_on(pool.get()).unwrap(), current_folder, user_id));
  if parent.is_none() { return false; }

  vec.push(parent.clone().unwrap());

  select_parent_folder_recursive(pool, parent.unwrap(), user_id, vec)
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

  Some(data)
}
