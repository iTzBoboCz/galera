use actix_web::{HttpResponse, get, web};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
struct User {
  id: u32,
  name: String,
  description: String,
}

// get /user/username/test
// #[get("/user/username/{name}")]
pub async fn test(name: web::Path<String>) -> HttpResponse {
  let user = User {
    id: 0,
    name: name.to_string(),
    description: ("test").to_string(),
  };

  let serialized = serde_json::to_string(&user).unwrap();

  HttpResponse::Ok()
    .content_type("application/json; charset=utf-8")
    .body(serialized)
}
