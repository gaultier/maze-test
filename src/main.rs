use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

#[derive(Debug, Serialize, Deserialize)]
struct CreateMazeHttpRequest {
    entrance: String,
    grid_size: String,
    walls: Vec<String>,
}

struct MazeCell {
    column: char,
    row: u8,
}

struct CreateMazeRequest {
    entrance: MazeCell,
    grid_size: (u8, u8),
    walls: Vec<MazeCell>,
}

impl TryFrom<&str> for MazeCell {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let parts: Vec<&str> = value.split_inclusive(char::is_alphabetic).collect();
        if parts.len() != 2 {
            return Err(Error {
                error: String::from("Malformed cell"),
            });
        }

        let c = parts[0].chars().nth(0);
        if c.is_none() {
            return Err(Error {
                error: String::from("Malformed cell"),
            });
        }

        let c = c.unwrap();
        if !c.is_uppercase() {
            return Err(Error {
                error: String::from("Malformed cell"),
            });
        }

        let row = parts[1].parse::<u8>();
        if row.is_err() {
            return Err(Error {
                error: format!("Malformed cell: {}", row.unwrap_err()),
            });
        }

        Ok(MazeCell {
            column: c,
            row: row.unwrap(),
        })
    }
}

impl TryFrom<&web::Json<CreateMazeHttpRequest>> for CreateMazeRequest {
    type Error = Error;

    fn try_from(value: &web::Json<CreateMazeHttpRequest>) -> Result<Self, Self::Error> {
        let coords: Vec<&str> = value.entrance.split('x').collect();
        if coords.len() != 2 {
            return Err(Error {
                error: String::from("Malformed entrance"),
            });
        }

        let grid_size_width = coords[0].parse::<u8>();
        if grid_size_width.is_err() {
            return Err(Error {
                error: format!("Malformed grid size: {}", grid_size_width.unwrap_err()),
            });
        }
        let grid_size_height = coords[1].parse::<u8>();
        if grid_size_height.is_err() {
            return Err(Error {
                error: format!("Malformed grid size: {}", grid_size_height.unwrap_err()),
            });
        }

        let entrance = value.entrance.as_str().try_into()?;

        let mut walls = Vec::with_capacity(value.walls.len());
        for wall in value.walls.as_slice().as_ref() {
            walls.push(wall.as_str().try_into()?);
        }

        Ok(CreateMazeRequest {
            entrance,
            grid_size: (grid_size_width.unwrap(), grid_size_height.unwrap()),
            walls,
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Error {
    error: String,
}

async fn create_maze(create_maze_http_req: web::Json<CreateMazeHttpRequest>) -> HttpResponse {
    let create_maze_req: CreateMazeRequest = match (&create_maze_http_req).try_into() {
        Err(err) => {
            return HttpResponse::BadGateway().json(err);
        },
        Ok(v) => v,
    };

//    let conn = match Connection::open_in_memory() {
//        Ok(conn) => conn,
//        Err(err) => {
//            return HttpResponse::BadGateway().json(Error {
//                error: err.to_string(),
//            })
//        }
//    };

    HttpResponse::Ok().json(3) // <- send response
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
