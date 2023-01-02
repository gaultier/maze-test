use actix_web::{middleware, web, App, HttpResponse, HttpServer};
//use rusqlite::{Connection, Result};
use serde::{Deserialize, Serialize};
use std::{collections::VecDeque, convert::TryFrom};

#[derive(Debug, Serialize, Deserialize)]
struct CreateMazeHttpRequest {
    entrance: String,
    #[serde(rename(deserialize = "gridSize"))]
    grid_size: String,
    walls: Vec<String>,
}

type Position = usize;

#[derive(Debug)]
struct Coord(usize, usize);

impl Coord {
    fn to_pos(self: &Self, width: usize) -> usize {
        return self.1 * width + self.0;
    }

    fn from_pos(pos: usize, width: usize) -> Self {
        let x = pos % width;
        let y = pos / width;
        return Coord(x, y);
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum MazeCellKind {
    Wall,
    Empty,
    Entry,
    Exit,
}

#[derive(Debug)]
struct CreateMazeRequest {
    entrance: Coord,
    grid_size: (u8, u8),
    walls: Vec<Coord>,
}

#[derive(Debug, Copy, Clone)]
struct Node {
    maze_pos: usize,
    parent_idx: Option<usize>,
}

impl TryFrom<&str> for Coord {
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

        let column = char_bytes[0] - 65;

        Ok(Coord(row as usize, column as usize))
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

fn bfs(maze: &[MazeCellKind], entrance_pos: usize, width: usize, height: usize) -> Option<(Vec<Node>, Node)> {
    let mut explored = Vec::with_capacity(width * height);
    explored.resize(explored.capacity(), false);

    let root_node = Node {
        maze_pos: entrance_pos,
        parent_idx: None,
    };
    let mut nodes: Vec<Node> = Vec::with_capacity(width * height);
    nodes.push(root_node);

    let mut work: VecDeque<usize> = VecDeque::new();
    work.push_front(0);

    explored[entrance_pos] = true;

    while !work.is_empty() {
        let work_node_idx = work.pop_front().unwrap();
        let work_node = nodes[work_node_idx];
        let work_pos = work_node.maze_pos;

        if maze[work_pos] == MazeCellKind::Exit {
            return Some((nodes, work_node));
        }

        let work_coord = Coord::from_pos(work_pos, width);
        let adjacent_cells: [(isize, isize); 4] = [
            (work_coord.0 as isize + 1, work_coord.1 as isize),
            (work_coord.0 as isize - 1, work_coord.1 as isize),
            (work_coord.0 as isize, work_coord.1 as isize + 1),
            (work_coord.0 as isize, work_coord.1 as isize - 1),
        ];

        for adjacent_cell in adjacent_cells {
            // Out of bounds
            if adjacent_cell.0 < 0
                || adjacent_cell.1 < 0
                || adjacent_cell.0 as usize >= width
                || adjacent_cell.1 as usize >= height
            {
                continue;
            }

            let adjacent_cell_pos =
                Coord(adjacent_cell.0 as usize, adjacent_cell.1 as usize).to_pos(width);

            if !explored[adjacent_cell_pos] {
                explored[adjacent_cell_pos] = true;
                let adjacent_cell_node = Node {
                    maze_pos: adjacent_cell_pos,
                    parent_idx: Some(work_node_idx),
                };
                nodes.push(adjacent_cell_node);
                work.push_back(nodes.len() - 1);
            }
        }
    }

    return None;
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
    maze.resize(maze.capacity(), MazeCellKind::Empty);

    for cell in create_maze_req.walls {
        let pos = cell.to_pos(width);
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

    let entrance_pos = create_maze_req.entrance.to_pos(width);
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

    let path = bfs(&maze, entrance_pos, width, height);
    println!("{:?}", path);

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
