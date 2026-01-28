#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::fs;
use std::sync::OnceLock;
use directories::ProjectDirs;
use thiserror::Error;
use tracing::debug;

static DIRS: OnceLock<Directories> = OnceLock::new();

#[derive(Debug, Clone, Copy, Error)]
pub enum RequiredDir {
  #[error("config")]
  Config,
  #[error("data")]
  Data,
}

#[derive(Debug, Error)]
pub enum DirError {
  #[error("could not determine OS directories (ProjectDirs::from returned None)")]
  UnknownHome,

  #[error("failed to create/check directory {path}: {source}")]
  Io {
    path: PathBuf,
    #[source]
    source: std::io::Error,
  },

  #[error("Directories::init() called more than once")]
  AlreadyInitialized,

  #[error("required directory '{kind}' failed: {source}")]
  RequiredDir {
    kind: RequiredDir,
    #[source]
    source: Box<DirError>,
  },
}

#[derive(Debug)]
pub struct Directories {
  config: PathBuf,
  data: PathBuf,
}

impl Directories {
  fn project_dirs() -> Result<ProjectDirs, DirError> {
    ProjectDirs::from("org", "galera", "galera").ok_or(DirError::UnknownHome)
  }

  pub fn init() -> Result<(), DirError> {
    let pd = Self::project_dirs()?;

    let config = Self::ensure_required_dir(RequiredDir::Config, pd.config_dir())?;
    let data = Self::ensure_required_dir(RequiredDir::Data, pd.data_dir())?;

    let dirs = Directories { config, data };

    DIRS.set(dirs).map_err(|_| DirError::AlreadyInitialized)?;

    Ok(())
  }

  pub fn get() -> &'static Directories {
    DIRS.get().expect("Directories::init() was not called")
  }

  pub fn data(&self) -> &PathBuf {
    &self.data
  }

  pub fn config(&self) -> &PathBuf {
    &self.config
  }

  pub fn gallery_dir(&self) -> Result<PathBuf, DirError> {
    let p = &self.data.join("gallery");
    Self::ensure_dir(&p)
  }

  fn ensure_dir(path: &Path) -> Result<PathBuf, DirError> {
    if path.is_dir() {
      return Ok(path.to_path_buf());
    }

    debug!("Trying to create a missing directory on path {:?}.", path);

    fs::create_dir_all(path).map_err(|e| DirError::Io {
      path: path.to_path_buf(),
      source: e,
    })?;

    debug!("Successfully created directory {:?}.", path);

    Ok(path.to_path_buf())
  }

  fn ensure_required_dir(kind: RequiredDir, path: &Path) -> Result<PathBuf, DirError> {
    Self::ensure_dir(path).map_err(|e| DirError::RequiredDir {
      kind,
      source: Box::new(e),
    })
  }
}
