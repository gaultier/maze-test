use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

#[derive(Debug, Serialize, Deserialize)]
struct CreateMazeHttpRequest {
    entrance: String,
    #[serde(rename(deserialize = "gridSize"))]
    grid_size: String,
    walls: Vec<String>,
}

#[derive(Debug)]
struct MazeCellCoord {
    column: u8,
    row: u8,
}

#[derive(Debug)]
enum MazeCellKind {
    Wall,
    Empty,
    Entry,
    Exit,
}

#[derive(Debug)]
struct CreateMazeRequest {
    entrance: MazeCellCoord,
    grid_size: (u8, u8),
    walls: Vec<MazeCellCoord>,
}

impl TryFrom<&str> for MazeCellCoord {
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
        if !c.is_ascii_uppercase() {
            return Err(Error {
                error: String::from("Malformed cell"),
            });
        }

        let mut char_bytes: [u8; 1] = [0; 1];
        c.encode_utf8(&mut char_bytes);

        let row = parts[1].parse::<u8>();
        if row.is_err() {
            return Err(Error {
                error: format!("Malformed cell: {}", row.unwrap_err()),
            });
        }
        let row = row.unwrap();
        if row == 0 {
            return Err(Error {
                error: format!("Malformed cell: should start at 1"),
            });
        }

        Ok(MazeCellCoord {
            column: char_bytes[0] - 65,
            row: row - 1,
        })
    }
}

impl TryFrom<&web::Json<CreateMazeHttpRequest>> for CreateMazeRequest {
    type Error = Error;

    fn try_from(value: &web::Json<CreateMazeHttpRequest>) -> Result<Self, Self::Error> {
        let coords: Vec<&str> = value.grid_size.split('x').collect();
        if coords.len() != 2 {
            return Err(Error {
                error: String::from("Malformed grid size"),
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
        }
        Ok(v) => v,
    };

    println!("{:?}", create_maze_req);

    let width = create_maze_req.grid_size.0 as usize;
    let height = create_maze_req.grid_size.1 as usize;
    let mut maze: Vec<MazeCellKind> = Vec::with_capacity(width * height);
    maze.resize_with(maze.capacity(), || MazeCellKind::Empty);

    for cell in create_maze_req.walls {
        let pos = cell.column as usize * width + cell.row as usize;
        maze[pos] = MazeCellKind::Wall;
    }

    let corners: [(usize, usize); 4] = [
        (0, 0),
        (width - 1, 0),
        (0, height - 1),
        (width - 1, height - 1),
    ];
    for corner in corners {
        let pos = corner.1 * width + corner.0;
        maze[pos] = MazeCellKind::Exit;
    }

    let entrance_pos =
        create_maze_req.entrance.column as usize * width + create_maze_req.entrance.row as usize;
    maze[entrance_pos] = MazeCellKind::Entry;
    println!("{:?}", maze);

    for y in 0..height {
        for x in 0..width {
            let pos = y * width + x;
            match maze[pos] {
                MazeCellKind::Wall => print!("x"),
                MazeCellKind::Empty => print!("."),
                MazeCellKind::Entry => print!(">"),
                MazeCellKind::Exit => print!("o"),
            }
        }
        println!("");
    }

    //for cell in create_maze_req.

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
