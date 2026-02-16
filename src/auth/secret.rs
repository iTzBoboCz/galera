use rand::{RngExt, distr::Alphanumeric, rng};
use std::{fs, path::PathBuf};

use crate::directories::Directories;

pub struct Secret {
  key: String,
}

impl Secret {
  const SECRET_LENGTH: usize = 512;

  fn path() -> PathBuf {
    let config = Directories::get().config();
    config.join("secret.key")
  }

  /// Generates a new secret.
  /// # Example
  /// ```
  /// let my_secret_string = Secret::generate();
  /// ```
  fn generate() -> String {
    let rng = rng();

    rng
      .sample_iter(&Alphanumeric)
      .take(Self::SECRET_LENGTH)
      .map(char::from)
      .collect()
  }

  /// Reads content of a secret.key file.
  pub fn read() -> Result<Secret, std::io::Error> {
    let raw = fs::read_to_string(Self::path())?;
    let key = raw.trim().to_string();
    if key.is_empty() {
      return Err(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "secret.key is empty",
      ));
    }

    Ok(Secret { key })
  }

  /// Writes a secret to the secret.key file.
  /// # Example
  /// ```
  /// // creates a new secret
  /// let my_secret = Secret::new();
  ///
  /// // writes it to the disk
  /// my_secret.write();
  /// ```
  pub fn write(self) -> std::io::Result<()> {
    let path = Self::path();
    let tmp = &path.with_extension("key.tmp"); // -> secret.key.tmp

    fs::write(tmp, self.key)?;
    fs::rename(tmp, path)?;

    Ok(())
  }

  /// Creates a new secret
  /// # Example
  /// ```
  /// let my_secret = Secret::new();
  /// ```
  pub fn new() -> Secret {
    Secret {
      key: Secret::generate()
    }
  }

  pub fn as_bytes(&self) -> &[u8] {
    self.key.as_bytes()
  }
}
