use axum::http::{HeaderMap};
use axum_extra::extract::{CookieJar, cookie::{Cookie, SameSite}};
use time::Duration;
use tracing::error;

pub const REFRESH_COOKIE: &str = "refresh_token";

/// Determine if the original request was HTTPS.
pub fn is_https(headers: &HeaderMap) -> bool {
  if let Some(v) = headers.get("x-forwarded-proto").and_then(|v| v.to_str().ok()) {
    return v.eq_ignore_ascii_case("https");
  }

  if let Some(v) = headers.get("forwarded").and_then(|v| v.to_str().ok()) {
    return v.to_ascii_lowercase().contains("proto=https");
  }

  false
}

/// Builds the refresh token cookie.
/// - Host-only (no Domain attribute)
/// - HttpOnly
/// - Secure if request is HTTPS
/// - Path-limited to refresh endpoint
pub fn build_refresh_cookie(refresh_token: String, headers: &HeaderMap) -> Cookie<'static> {
  let mut c = Cookie::new(REFRESH_COOKIE, refresh_token);

  c.set_http_only(true);
  c.set_secure(is_https(headers));
  error!("{} - {:?}", is_https(headers), headers);
  c.set_same_site(SameSite::Lax);

  c.set_path("/auth/");

  c.set_max_age(Duration::days(30));

  c
}

pub fn clear_refresh_cookie(headers: &HeaderMap) -> Cookie<'static> {
  let mut c = Cookie::new(REFRESH_COOKIE, "");

  c.set_http_only(true);
  c.set_secure(is_https(headers));
  c.set_same_site(SameSite::Lax);
  c.set_path("/auth/");
  c.set_max_age(Duration::seconds(0));

  c
}

pub fn read_refresh_token(jar: &CookieJar) -> Option<String> {
  jar.get(REFRESH_COOKIE).map(|c| c.value().to_string())
}
