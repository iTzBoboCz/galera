use reqwest::Url;

/// Returns normalised BACKEND_URL
pub fn get_backend_url() -> Option<Url> {
  let raw = std::env::var("BACKEND_URL")
    .ok()
    .map(|s| s.trim().to_string())
    .filter(|s| !s.is_empty())?;

  normalise_backend_url(&raw)
}

fn normalise_backend_url(backend_url: &str) -> Option<Url> {
  let Ok(mut url) = Url::parse(backend_url) else {
    return None;
  };

  let path = url
    .path()
    .split('/')
    .filter(|s| !s.is_empty())
    .collect::<Vec<_>>()
    .join("/");

  let normalized_path = if path.is_empty() {
    '/'.to_string()
  } else {
    format!("/{}/", path)
  };


  url.set_path(&normalized_path);

  Some(url)
}

#[test]
fn localhost() {
  let url = normalise_backend_url("http://localhost:8000").unwrap();
  assert_eq!(url.as_str(), "http://localhost:8000/");
}

#[test]
fn normalizes_extra_slashes() {
  let url = normalise_backend_url("https://galera.test.local///api////").unwrap();
  assert_eq!(url.as_str(), "https://galera.test.local/api/");
}

#[test]
fn adds_trailing_slash() {
  let url = normalise_backend_url("https://galera.test.local/api").unwrap();
  assert_eq!(url.as_str(), "https://galera.test.local/api/");
}

#[test]
fn accepts_root_path() {
  let url = normalise_backend_url("https://galera.test.local/").unwrap();
  assert_eq!(url.as_str(), "https://galera.test.local/");
}

#[test]
fn rejects_invalid_url() {
  assert!(normalise_backend_url("not a url").is_none());
}
