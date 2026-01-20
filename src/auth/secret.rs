use rand::{Rng, distr::Alphanumeric, rng};
use std::fs;

pub struct Secret {
  key: String,
}

impl Secret {
  /// Generates a new secret.
  /// # Example
  /// ```
  /// let my_secret_string = Secret::generate();
  /// ```
  fn generate() -> String {
    let mut rng = rng();

    let range = rng.random_range(256..512);

    String::from_utf8(
      rng
        .sample_iter(&Alphanumeric)
        .take(range)
        .collect::<Vec<_>>(),
    )
    .unwrap()
  }

  /// Reads content of a secret.key file.
  // TODO: check for write and read permissions
  pub fn read() -> Result<String, std::io::Error> {
    let path = "secret.key";
    fs::read_to_string(path)
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
    let path = "secret.key";
    fs::write(path, self.key)
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
}
