use actix_web::{middleware, web, App, HttpResponse, HttpServer};
// use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct CreateMazeRequest {
    name: String,
    number: i32,
}

async fn create_maze(create_maze_req: web::Json<CreateMazeRequest>) -> HttpResponse {
    //let conn = Connection::open_in_memory()?;
    HttpResponse::Ok().json(create_maze_req) // <- send response
}

#[actix_web::main] // or #[tokio::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new()
            // enable logger
            .wrap(middleware::Logger::default())
            .app_data(web::JsonConfig::default().limit(4096)) // <- limit size of the payload (global configuration)
            .service(web::resource("/maze").route(web::post().to(create_maze)))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
