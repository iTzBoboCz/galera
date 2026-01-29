use crate::{db, ConnectionPool};
use crate::models::{Folder, NewFolder};

use tracing::{error, info, warn, trace, debug};
use std::collections::HashMap;
use std::fs;
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};

const MAX_DEPTH: usize = 128;

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

  let Ok(Some(kind)) = infer::get_from_path(pathbuf) else {
    return false;
  };

  if valid_mime_types.contains(&kind.mime_type()) {
    trace!("Found: {:?} with type: {:?}", pathbuf, kind.mime_type());

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

  let folder_ids = add_folders_to_db(pool.clone(), found_folders.clone(), user_id).await;

  for (folder, folder_id) in folder_ids.into_iter() {
    scan_folder_for_media(pool.clone(), xdg_data.join(folder), folder_id, user_id).await;
  }

  info!("Scanning is done.");
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

pub async fn scan_folder_for_media(pool: ConnectionPool, absolute_path: PathBuf, folder_id: i32, user_id: i32) {
  let Some(parent_folder) = db::folders::select_folder(pool.get().await.unwrap(), folder_id).await else {
    warn!("Folder id {} not found in DB (user_id={})", folder_id, user_id);
    return;
  };

  let Some(media_scanned_vec) = folder_get_media(absolute_path) else {
    return;
  };

  if media_scanned_vec.is_empty() { return; }

  for media_scanned in media_scanned_vec {
    let name = media_scanned.file_name().unwrap().to_str().unwrap().to_owned();

    let media = db::media::check_if_media_present(pool.get().await.unwrap(), name.clone(), parent_folder.clone(), user_id)
      .await;

    if media.is_none() {
      debug!("{:?} doesnt exist in database", media_scanned);

      let Ok(image_dimensions) = imagesize::size(media_scanned.clone()) else {
        warn!("Image {:?} was skipped as its dimensions are unknown.", media_scanned);
        continue;
      };

      db::media::insert_media(pool.get().await.unwrap(), name, parent_folder.clone(), user_id,  (image_dimensions.width.try_into().unwrap(), image_dimensions.height.try_into().unwrap()), None, media_scanned)
      .await;
    }
  }
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
