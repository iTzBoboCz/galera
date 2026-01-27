use std::io::{self, Read};
use sha2::{Digest, Sha512};

pub fn sha2512_hash<R: Read>(reader: &mut R) -> io::Result<String> {
  let mut hasher = Sha512::new();
  let mut buffer = [0u8; 4096];

  loop {
    let n = reader.read(&mut buffer)?;
    if n == 0 {
      break;
    }
    hasher.update(&buffer[..n]);
  }

  let digest = hasher.finalize();
  Ok(hash_string(&digest))
}

pub fn hash_string(bytes: &[u8]) -> String {
  let mut result = String::with_capacity(bytes.len() * 2);
  for b in bytes {
    use std::fmt::Write;
    write!(result, "{:02X}", b).unwrap();
  }
  result
}
