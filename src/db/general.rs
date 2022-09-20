use crate::DbConn;
use diesel::select;
use diesel::sql_types::Integer;
use diesel::OptionalExtension;
use diesel::RunQueryDsl;

/// Returns last inserted id.
/// # Example
/// We inserted a new folder and we need its ID.
/// ```
/// insert_folder(conn, new_folder, name, path).await;
///
/// let folder_id: Option<i32> = get_last_insert_id(&conn);
/// ```
pub async fn get_last_insert_id(conn: DbConn) -> Option<i32> {
  conn.interact(|c| {
    no_arg_sql_function!(last_insert_id, Integer);

    select(last_insert_id)
      .first(c)
      .optional()
      .unwrap()
  }).await.unwrap()
}
