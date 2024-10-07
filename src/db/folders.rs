use crate::models::{Folder, NewFolder};
use crate::schema::folder;
use crate::DbConn;
use diesel::BoolExpressionMethods;
use diesel::ExpressionMethods;
use diesel::OptionalExtension;
use diesel::QueryDsl;
use diesel::RunQueryDsl;
use diesel::Table;
use std::path::PathBuf;

pub async fn insert_folder(conn: DbConn, new_folder: NewFolder, name: String, path: PathBuf) {
  let _ = conn.interact(move |c| {
    diesel::insert_into(folder::table)
      .values(new_folder)
      .execute(c)
      .unwrap_or_else(|_| panic!("Error scanning folder {} in {}", name, path.display().to_string()));
  }).await;
}

pub async fn select_child_folder_id(conn: DbConn, name: String, parent: Option<i32>, user_id: i32) -> Option<i32> {
  if parent.is_none() {
    conn.interact(move |c| {
      folder::table
        .select(folder::id)
        .filter(folder::dsl::parent.is_null().and(folder::dsl::name.eq(name).and(folder::owner_id.eq(user_id))))
        .first::<i32>(c)
        .optional()
        .unwrap()
    }).await.unwrap()

  } else {
    conn.interact(move |c| {
      folder::table
        .select(folder::id)
        .filter(folder::dsl::parent.eq(parent).and(folder::dsl::name.eq(name).and(folder::owner_id.eq(user_id))))
        .first::<i32>(c)
        .optional()
        .unwrap()
    }).await.unwrap()
  }
}

pub async fn select_root_folder(conn: DbConn, user_id: i32) -> Result<Option<Folder>, diesel::result::Error> {
  conn.interact(move |c| {
    folder::table
      .select(folder::table::all_columns())
      .filter(folder::dsl::parent.is_null().and(folder::owner_id.eq(user_id)))
      .first::<Folder>(c)
      .optional()
  }).await.unwrap()
}

pub async fn select_subfolders(conn: DbConn, parent_folder: Folder, user_id: i32) -> Vec<Folder> {
  conn.interact(move |c| {
    folder::table
      .select(folder::table::all_columns())
      .filter(folder::dsl::parent.eq(parent_folder.id).and(folder::owner_id.eq(user_id)))
      .get_results::<Folder>(c)
      .optional()
      .unwrap()
      .unwrap()
  }).await.unwrap()
}

/// Selects folder from folder id.
/// # Example
/// We're selecting folder with id 10.
/// ```
/// let folder: Folder = select_folder(&conn, 10);
/// ```
pub async fn select_folder(conn: DbConn, folder_id: i32) -> Option<Folder> {
  conn.interact(move |c| {
    folder::table
      .select(folder::table::all_columns())
      .filter(folder::dsl::id.eq(folder_id))
      .first::<Folder>(c)
      .optional()
      .unwrap()
  }).await.unwrap()
}

/// Selects parent folder.
/// # Example
/// We're selecting parent folder of a folder with id 10, where user id is 1.
/// ```
/// let current_folder: Folder = select_folder(&conn, 10);
/// let parent_folder: Option<Folder> = select_parent_folder(&conn, current_folder, 1);
/// ```
pub async fn select_parent_folder(conn: DbConn, current_folder: Folder, user_id: i32) -> Option<Folder> {
  conn.interact(move |c| {
    folder::table
      .select(folder::table::all_columns())
      .filter(folder::dsl::id.eq(current_folder.parent?).and(folder::owner_id.eq(user_id)))
      .first::<Folder>(c)
      .ok()
  }).await.unwrap()
}

pub async fn select_folder_id(conn: DbConn, folder_uuid: String) -> Option<i32> {
  conn.interact(move |c| {
    folder::table
      .select(folder::id)
      .filter(folder::dsl::uuid.eq(folder_uuid))
      .first::<i32>(c)
      .optional()
      .unwrap()
  }).await.unwrap()
}
