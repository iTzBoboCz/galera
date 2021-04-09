use std::{fs, io};
use std::path::{PathBuf, Path};
use infer;

pub fn is_media_suppoted(pathbuf: std::path::PathBuf) -> bool {
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
    "audio/aac"
  ];

  // let kind = infer::get_from_path(pathbuf).unwrap();
  // let kind = infer::get_from_path(pathbuf.as_path()).unwrap();
  // if !kind.is_none() && valid_mime_types.contains(&kind.unwrap().mime_type()) {
  //   return true;
  // }

  return false;
}

pub fn folder_has_media(dir: PathBuf) -> bool {
  let data: Vec<PathBuf> = fs::read_dir(&dir).unwrap()
    .into_iter()
    .filter(|r| r.is_ok()) // Get rid of Err variants for Result<DirEntry>
    .map(|r| r.unwrap().path()) // This is safe, since we only have the Ok variants
    .filter(|r| is_media_suppoted(PathBuf::from(r)) == false) // Filter out non-folders
    .collect();

  for x in &data {
    warn!("{:?}", x);
  }

  if data.len() > 0 {
    return true;
  }

  return false;
}
