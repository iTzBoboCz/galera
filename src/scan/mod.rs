use std::fs;
use std::path::PathBuf;
use infer;

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

pub fn scan_recursively(path: PathBuf) -> Vec<PathBuf> {
  // skip empty folders
  if path.read_dir().map(|mut i| i.next().is_none()).unwrap_or(false) { return Vec::new() }

  let folders = fs::read_dir(path.clone()).unwrap()
    .into_iter()
    .filter(|r| r.is_ok()) // Get rid of Err variants for Result<DirEntry>
    .map(|r| r.unwrap().path()) // This is safe, since we only have the Ok variants
    .filter(|r| r.is_dir())
    .collect::<Vec<PathBuf>>();

  let mut array: Vec<PathBuf> = Vec::new();
  for folder in folders.clone() {
    array.append(&mut scan_recursively(folder));
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
  }

  return array;
}

pub fn scan_root(xdg_data: &str, username: &str) {
  // root directory
  let current_dir = format!("{}/{}/", xdg_data, username);

  let mut found_folders: Vec<PathBuf> = Vec::new();

  info!("Scanning files and folders for user {} started.", username);

  let folders = fs::read_dir(current_dir.clone()).unwrap()
    .into_iter()
    .filter(|r| r.is_ok()) // Get rid of Err variants for Result<DirEntry>
    .map(|r| r.unwrap().path()) // This is safe, since we only have the Ok variants
    .collect::<Vec<PathBuf>>();

  if folders.len() > 0 {
    found_folders.append(&mut scan_recursively(PathBuf::from(current_dir)));
  }

  debug!("{:?}", found_folders);
  info!("Scanning is done.");
}

