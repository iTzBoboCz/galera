#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::fs;
use directories::ProjectDirs;
use tracing::{error, info, warn};

#[derive(Debug)]
pub struct Directories {
  config: PathBuf,
  data: PathBuf,
}

// TODO: redo this with errors instead of options
impl Directories {
  fn get_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from("org", "galera", "galera")
  }

  pub fn data(&self) -> &PathBuf {
    &self.data
  }

  pub fn config(&self) -> &PathBuf {
    &self.config
  }

  pub fn gallery(&self) -> Option<PathBuf> {
    let path = &self.data.join("gallery");

    Directories::check(path)
  }

  pub fn new() -> Option<Directories> {
    let dirs_option = Directories::get_dirs();
    if dirs_option.is_none() {
      error!("Home directory location is unknown.");
      return None;
    }

    let dirs = dirs_option.unwrap();

    Some(Directories {
      config: Directories::check(dirs.config_dir())?,
      data: Directories::check(dirs.data_dir())?
    })
  }

  fn check(path: &Path) -> Option<PathBuf> {
    if !path.is_dir() {
      warn!("Trying to create a missing directory on path {:?}.", path);
      let created = fs::create_dir_all(path);

      if created.is_err() {
        error!("Missing folder could not be created.");
        return None;
      }

      info!("Folder created successfully.");
    }

    Some(PathBuf::from(path))
  }
}
