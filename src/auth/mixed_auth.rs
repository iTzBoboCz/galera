#![allow(dead_code)]

use std::sync::Arc;

use axum::{body::Body, extract::{FromRequestParts, Request, State}, http::{StatusCode, request::Parts}, middleware::Next, response::Response};
use axum_extra::{TypedHeader, headers::{Authorization, authorization}};

use crate::{AppState, auth::{shared_album_link::{SharedAlbumLinkSecurity, shared_album_link_validate}, token::{Claims, ClaimsEncoded}}};

#[derive(Debug, Clone)]
pub struct MixedAuth {
  pub claims: Option<Arc<Claims>>,
  pub shared_album_link: Option<Arc<SharedAlbumLinkSecurity>>,
}

impl MixedAuth {
  pub fn claims(claims: Arc<Claims>) -> Self {
    Self { claims: Some(claims), shared_album_link: None }
  }

  pub fn shared_album_link(link: Arc<SharedAlbumLinkSecurity>) -> Self {
    Self { claims: None, shared_album_link: Some(link) }
  }

  pub fn is_authenticated(&self) -> bool {
    self.claims.is_some() || self.shared_album_link.is_some()
  }
}

#[derive(Debug, Clone)]
pub enum AuthHeader {
  Bearer(Arc<Claims>),
  SharedAlbumLink(Arc<SharedAlbumLinkSecurity>),
}

impl FromRequestParts<AppState> for AuthHeader {
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &AppState) -> Result<Self, Self::Rejection> {
        // try Bearer first
        if let Ok(TypedHeader(Authorization(b))) =
            TypedHeader::<Authorization<authorization::Bearer>>::from_request_parts(parts, _state).await
        {
            return Ok(AuthHeader::Bearer(Arc::new(ClaimsEncoded(b.token().to_string()).decode().map_err(|_| StatusCode::UNAUTHORIZED)? .claims)));
        }

        // try Basic
        if let Ok(TypedHeader(Authorization(b))) =
            TypedHeader::<Authorization<authorization::Basic>>::from_request_parts(parts, _state).await
        {
            return Ok(AuthHeader::SharedAlbumLink(Arc::new(SharedAlbumLinkSecurity {
                album_share_link_uuid: b.username().to_string(),
                password: Some(b.password().to_string()),
            })));
        }

        Err(StatusCode::UNAUTHORIZED)
    }
}

impl AuthHeader {
    pub fn claims(&self) -> Option<&Claims> {
        match self { Self::Bearer(c) => Some(c), _ => None }
    }

    pub fn shared_link(&self) -> Option<&SharedAlbumLinkSecurity> {
        match self { Self::SharedAlbumLink(s) => Some(s), _ => None }
    }
}

pub async fn mixed_auth(
    State(state): State<AppState>,
    auth: AuthHeader,
    mut req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let mixed = match auth {
        AuthHeader::Bearer(token) => {
            if !token.is_valid(state.pool.clone()).await {
                return Err(StatusCode::UNAUTHORIZED);
            }
            MixedAuth::claims(token.clone())
        }

        AuthHeader::SharedAlbumLink(shared_album_link) => {
            let link = shared_album_link_validate(
                state.pool,
                shared_album_link.album_share_link_uuid(),
                shared_album_link.password().unwrap_or("".to_owned()),
            )
            .await
            .map_err(|_| StatusCode::UNAUTHORIZED)?;

            MixedAuth::shared_album_link(Arc::new(link))
        }
    };

    req.extensions_mut().insert(Arc::new(mixed));
    Ok(next.run(req).await)
}
