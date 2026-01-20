#![allow(unused_qualifications)]

pub mod albums;
pub mod folders;
pub mod media;
pub mod tokens;
pub mod users;
pub mod oidc;

/// Used for getting last inserted id.
#[derive(diesel::deserialize::QueryableByName)]
pub struct LastInsertId {
  #[diesel(sql_type = diesel::sql_types::Integer)]
  pub id: i32,
}
