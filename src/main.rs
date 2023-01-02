use actix_web::{post, App, HttpServer, Responder};

#[post("/maze")]
async fn create_maze() -> impl Responder {
    format!("Hello!")
}

#[actix_web::main] // or #[tokio::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        App::new().service(create_maze)
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
