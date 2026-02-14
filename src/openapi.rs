use utoipa::{Modify, OpenApi, openapi::{PathItem, Server}};

pub mod tags {
  // Authentication tags
  pub const AUTH_PUBLIC: &str = "auth:public";
  pub const AUTH_PROTECTED: &str = "auth:protected";
  pub const AUTH_MIXED: &str = "auth:mixed";

  // Route domain tags
  pub const ALBUMS: &str = "albums";
  pub const AUTH: &str = "auth";
  pub const MEDIA: &str = "media";
  pub const OIDC: &str = "oidc";
  pub const OTHER: &str = "other";
}

#[derive(OpenApi)]
#[openapi(
  tags(
    (name = tags::AUTH_PUBLIC, description = "Public endpoints without authentication"),
    (name = tags::AUTH_PROTECTED, description = "Protected endpoints using `BearerAuth`"),
    (name = tags::AUTH_MIXED, description = "Protected endpoints using `BearerAuth` or `BasicSharedAlbumLinkAuth`"),
  ),
  paths(
    crate::routes::media::media_structure,
    crate::routes::media::get_media_by_uuid,
    crate::routes::media::media_update_description,
    crate::routes::media::media_delete_description,
    crate::routes::media::get_media_liked_list,
    crate::routes::media::media_like,
    crate::routes::media::media_unlike,
    crate::routes::albums::create_album,
    crate::routes::albums::album_add_media,
    crate::routes::albums::get_album_list,
    crate::routes::albums::update_album,
    crate::routes::albums::delete_album,
    crate::routes::albums::get_album_share_links,
    crate::routes::albums::create_album_share_link,
    crate::routes::albums::update_album_share_link,
    crate::routes::albums::delete_album_share_link,
    crate::routes::scan_media,
    crate::routes::health,
    crate::routes::create_user,
    crate::routes::login,
    crate::routes::logout,
    crate::routes::refresh_token,
    crate::routes::system_info_public,
    crate::routes::oidc::get_server_config,
    crate::routes::oidc::oidc_login,
    crate::routes::oidc::oidc_callback,
    crate::routes::albums::get_album_share_link,
    crate::routes::albums::get_album_structure
  ),
  modifiers(&BearerSecurityAddon, &OperationIdPrefix, &ServerPrefix)
)]
pub struct ApiDoc;

impl ApiDoc {
  pub fn generate_openapi() -> utoipa::openapi::OpenApi {
    Self::openapi()
  }

  pub fn generate_openapi_tagless() -> utoipa::openapi::OpenApi {
    let mut doc = ApiDoc::openapi();
    StripTags.modify(&mut doc);
    doc
  }
}

struct BearerSecurityAddon;

impl Modify for BearerSecurityAddon {
  fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
    use utoipa::openapi::security::{HttpAuthScheme, HttpBuilder, SecurityScheme};

    openapi.components.as_mut().unwrap().add_security_scheme(
      "BearerAuth",
      SecurityScheme::Http(
        HttpBuilder::new()
          .scheme(HttpAuthScheme::Bearer)
          .bearer_format("JWT")
          .description(Some("requires a bearer token to access"))
          .build(),
      ),
    );

    openapi.components.as_mut().unwrap().add_security_scheme(
      "BasicSharedAlbumLinkAuth",
      SecurityScheme::Http(
        HttpBuilder::new()
          .scheme(HttpAuthScheme::Basic)
          .description(Some(
            "requires a base64 encoded string in format `album_share_link_uuid:password` to access",
          ))
          .build(),
      ),
    );
  }
}

pub struct OperationIdPrefix;

impl Modify for OperationIdPrefix {
  fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
    for (_path, item) in openapi.paths.paths.iter_mut() {
      add_prefix(item, "routes_");
    }
  }
}

fn add_prefix(item: &mut PathItem, prefix: &str) {
  let ops = [
    &mut item.get,
    &mut item.post,
    &mut item.put,
    &mut item.delete,
    &mut item.patch,
    &mut item.options,
    &mut item.head,
    &mut item.trace,
  ];

  for op in ops {
    if let Some(op) = op {
      if let Some(id) = &op.operation_id {
        if !id.starts_with(prefix) {
          op.operation_id = Some(format!("{prefix}{id}"));
        }
      }
    }
  }
}

pub struct StripTags;

impl Modify for StripTags {
  fn modify(&self, doc: &mut utoipa::openapi::OpenApi) {
    for (_, path_item) in doc.paths.paths.iter_mut() {
      for op in [
        &mut path_item.get,
        &mut path_item.post,
        &mut path_item.put,
        &mut path_item.delete,
        &mut path_item.patch,
        &mut path_item.options,
        &mut path_item.head,
        &mut path_item.trace,
      ] {
        if let Some(op) = op {
          op.tags = None;
        }
      }
    }
    doc.tags = None;
  }
}

struct ServerPrefix;

impl Modify for ServerPrefix {
  fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
    // Preferred prefix from BACKEND_URL path ("/" or "/api/")
    let preferred = crate::config::get_backend_url()
      .map(|u| u.path().to_string())
      .unwrap_or_else(|| "/".to_string());

    let preferred = if preferred == "/" {
      "/".to_string()
    } else {
      preferred.trim_end_matches('/').to_string() // "/api"
    };

    // Always offer both for Swagger UI usability
    let a = Server::new(preferred.as_str());
    let b = if preferred == "/api" { Server::new("/") } else { Server::new("/api") };

    openapi.servers = Some(vec![a, b]);
  }
}
