use std::thread;
use diesel::{RunQueryDsl, query_dsl::methods::SelectDsl};
// use diesel::query_dsl::methods::Fi

use crate::DbConn;
use crate::scan;

#[openapi]
#[get("/")]
pub async fn index() -> &'static str {
  "Hello, world!"
}

// https://api.rocket.rs/master/rocket/struct.State.html
#[openapi]
#[get("/scan_media")]
pub async fn scan_media(conn: DbConn) -> &'static str {
  let xdg_data = "gallery";
  let user_id: i32 = 1;

  // let now_future = Delay::new(Duration::from_secs(10));

  // this thread will run until scanning is complete
  // thread::spawn(|conn, xdg_data, user_id| async {
  scan::scan_root(&conn, xdg_data, user_id).await;
  // });

  "true"
}

#[openapi]
#[get("/media/<media_id>")]
pub async fn get_media_by_id(conn: DbConn, media_id: String) -> String {
  media_id
}

#[openapi]
#[get("/test")]
pub async fn test(conn: DbConn) -> String {
  let media: i32 = conn.run(|c| {
  // check wheter the file is already in a database
  return crate::schema::media::table
    .select(crate::schema::media::id)
    .first::<i32>(c)
    .unwrap();
  }).await;

  media.to_string()
}
