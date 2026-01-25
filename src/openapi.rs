use utoipa::OpenApi;

pub const AUTH_PUBLIC: &str = "auth:public";
pub const AUTH_PROTECTED: &str = "auth:protected";
pub const AUTH_MIXED: &str = "auth:mixed";

#[derive(OpenApi)]
#[openapi(
  tags(
    (name = AUTH_PUBLIC, description = "Public endpoints without authentication"),
    (name = AUTH_PROTECTED, description = "Protected endpoints using `BearerAuth`"),
    (name = AUTH_MIXED, description = "Protected endpoints using `BearerAuth` or `BasicSharedAlbumLinkAuth`"),
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
    crate::routes::refresh_token,
    crate::routes::system_info_public,
    crate::routes::oidc::get_server_config,
    crate::routes::oidc::oidc_login,
    crate::routes::oidc::oidc_callback,
    crate::routes::albums::get_album_share_link,
    crate::routes::albums::get_album_structure
  ),
  modifiers(&BearerSecurityAddon)
)]
pub struct ApiDoc;

struct BearerSecurityAddon;

impl utoipa::Modify for BearerSecurityAddon {
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
