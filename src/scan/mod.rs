use crate::{db, ConnectionPool};
use crate::models::{Folder, NewFolder};

use thiserror::Error;
use tracing::{error, info, warn, trace, debug};
use std::collections::HashMap;
use std::{fs, io};
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};

const MAX_DEPTH: usize = 128;

#[derive(Debug, Error)]
pub enum ScanError {
  #[error("failed to create user root: {0}")]
  CreateUserRoot(io::Error),

  #[error("failed to read directory: {0}")]
  ReadDir(#[from] io::Error),
}

const SUPPORTED_MIME_TYPES: &[&str] = &[
  "image/jpeg",
  "image/png",
  // "image/gif",
  "image/webp",
  // "image/x-canon-cr2",
  // "image/tiff",
  // "image/bmp",
  // "image/heif",
  // "image/avif",
  // "video/mp4",
  // "video/x-m4v",
  // "video/x-matroska",
  // "video/webm",
  // "video/quicktime",
  // "video/x-msvideo",
  // "video/x-ms-wmv",
  // "video/mpeg",
  // "video/x-flv",
  // "audio/midi",
  // "audio/mpeg",
  // "audio/m4a",
  // "audio/ogg",
  // "audio/x-flac",
  // "audio/x-wav",
  // "audio/amr",
  // "audio/aac",
];

/// checks if the file type is supported.
/// returns **true** for example for **image/jpeg**
/// and **false** for **text/json**
pub fn is_media_supported(path: &Path) -> bool {
  let Ok(Some(kind)) = infer::get_from_path(path) else {
    return false;
  };

  let mime = kind.mime_type();
  let ok = SUPPORTED_MIME_TYPES.contains(&mime);

  if ok {
    trace!("Found: {:?} with type: {:?}", path, mime);
  }

  ok
}

/// Outputs a list of populated folders.
// TODO: find out which is better: strip -> sort vs sort -> strip
pub fn get_folders(directory: &Path, username: &str) -> Vec<PathBuf> {
  let mut dirs = vec![];

  let root = directory.join(username);

  let walker = walkdir::WalkDir::new(&root)
    .follow_links(false)
    .into_iter();

  for entry in walker {
    let entry = match entry {
      Ok(en) => en,
      Err(e) => {
        warn!("walkdir error: {}", e);
        continue;
      },
    };

    let path = entry.into_path();
    if !path.is_file() { continue; }

    let Some(parent) = path.parent() else { continue; };

    let strip = match parent.strip_prefix(directory) {
      Ok(p) => p.to_path_buf(),
      Err(_) => continue,
    };
    dirs.push(strip);
  }

  dirs.sort();
  dirs.dedup();

  dirs
}

/// scans folder of a given user
pub async fn scan_root(pool: ConnectionPool, xdg_data: PathBuf, user_id: i32) -> Result<(), ScanError> {
  // root directory
  let Some(username) = db::users::get_user_username(pool.get().await.unwrap(), user_id).await else {
    debug!("Scan skipped: user id {} doesn't exist.", user_id);
    return Ok(());
  };

  let user_root = xdg_data.join(username.clone());

  info!("Scanning files and folders for user {} started.", username);

  if !user_root.exists() {
    create_dir_all(&user_root).map_err(ScanError::CreateUserRoot)?;
  }

  let found_folders = get_folders(&xdg_data, &username);

  let folder_ids = add_folders_to_db(pool.clone(), found_folders, user_id).await;

  for (folder, folder_id) in folder_ids.into_iter() {
    let abs = xdg_data.join(folder);
    let _ = scan_folder_for_media(pool.clone(), &abs, folder_id, user_id).await;
  }

  info!("Scanning is done.");
  Ok(())
}

#[derive(Debug, Default, Clone)]
pub struct FolderIndex(HashMap<PathBuf, i32>);

#[allow(dead_code)]
impl FolderIndex {
  pub fn insert(&mut self, path: PathBuf, id: i32) {
    self.0.insert(path, id);
  }

  pub fn get(&self, path: &PathBuf) -> Option<i32> {
    self.0.get(path).copied()
  }

  pub fn iter(&self) -> impl Iterator<Item = (&PathBuf, &i32)> {
    self.0.iter()
  }

  pub fn into_iter(self) -> impl Iterator<Item = (PathBuf, i32)> {
    self.0.into_iter()
  }
}

// folders when using NTFS can be max. 260 characters (we currently support max. 255 - Linux maximum and max. VARCHAR size) TODO: warn user when scanning folder that is longer and skip it
pub async fn add_folders_to_db(pool: ConnectionPool, relative_paths: Vec<PathBuf>, user_id: i32) -> FolderIndex {
  let mut folder_ids = FolderIndex::default();

  for path in relative_paths {
    debug!("scanning path: {:?}", path);

    let string_split = path.to_str().unwrap().split('/').into_iter().map(|f| f.to_owned());

    let mut parent: Option<i32> = None;
    for (i, dirname) in string_split.enumerate() {
      if i == 0 {
        parent = None;
      }

      let folder_id = match db::folders::select_child_folder_id(pool.get().await.unwrap(), dirname.clone(), parent, user_id).await {
        Some(id) => id,
        None => {
          let new_folder = NewFolder::new(user_id, dirname.clone(), parent);

          let Ok(last_insert_id) = db::folders::insert_folder(pool.get().await.unwrap(), new_folder).await else {
            error!("Error scanning folder {} in {}", dirname, path.display().to_string());
            continue;
          };

          last_insert_id
        },
      };

      parent = Some(folder_id);
    }

    if let Some(folder_id) = parent {
      folder_ids.insert(path, folder_id);
    }
  }

  folder_ids
}

pub async fn scan_folder_for_media(pool: ConnectionPool, absolute_path: &Path, folder_id: i32, user_id: i32) -> Result<(), ScanError> {
  let Some(parent_folder) = db::folders::select_folder(pool.get().await.unwrap(), folder_id).await else {
    warn!("Folder id {} not found in DB (user_id={})", folder_id, user_id);
    return Ok(());
  };

  let media_scanned_vec = folder_get_media(&absolute_path)
    .map_err(ScanError::ReadDir)?;

  if media_scanned_vec.is_empty() { return Ok(()); }

  for media_scanned in media_scanned_vec {
    let Some(name) = media_scanned.file_name().and_then(|n| n.to_str()).map(|s| s.to_owned()) else {
      continue;
    };

    let exists = db::media::check_if_media_present(pool.get().await.unwrap(), name.clone(), parent_folder.clone(), user_id)
      .await
      .is_some();
    if exists { continue; };

    debug!("{:?} doesnt exist in database", media_scanned);

    let Ok(image_dimensions) = imagesize::size(media_scanned.clone()) else {
      warn!("Image {:?} was skipped as its dimensions are unknown.", media_scanned);
      continue;
    };

    let width: u32 = match image_dimensions.width.try_into() {
      Ok(v) => v,
      Err(_) => continue,
    };
    let height: u32 = match image_dimensions.height.try_into() {
      Ok(v) => v,
      Err(_) => continue,
    };

    db::media::insert_media(
      pool.get().await.unwrap(),
      name,
      parent_folder.clone(),
      user_id,
      (width, height),
      None,
      media_scanned
    )
    .await;
  }

  Ok(())
}

/// Selects parent folders.
// TODO: Write faster recursive function with diesel's sql_query()
pub async fn select_parent_folders(pool: ConnectionPool, mut current_folder: Folder, user_id: i32) -> Result<Vec<Folder>, ()> {
  let mut output = vec![current_folder.clone()];

  for _ in 0..MAX_DEPTH {
    let Some(parent) = db::folders::select_parent_folder(pool.get().await.unwrap(), current_folder.clone(), user_id).await else {
      // no more deeper folders found, return
      return Ok(output);
    };

    output.push(parent.clone());
    current_folder = parent;
  }

  // depth error or db error
  Err(())
}

pub fn folder_get_media(dir: &Path) -> Result<Vec<PathBuf>, io::Error> {
  let read_dir = fs::read_dir(dir)?;

  let data: Vec<PathBuf> = read_dir
    .into_iter()
    .flatten() // Get rid of Err variants for Result<DirEntry>
    .map(|r| r.path())
    .filter(|r| r.is_file())
    .filter(|r| is_media_supported(r)) // Filter out non-folders
    .collect();

  Ok(data)
}
