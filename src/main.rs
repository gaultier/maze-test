use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::convert::TryFrom;
use std::fmt;

#[derive(Debug, Serialize, Deserialize)]
struct CreateMazeHttpRequest {
    entrance: String,
    #[serde(rename(deserialize = "gridSize"))]
    grid_size: String,
    walls: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct CreateMaze {
    entrance: Coord,
    grid_size: (usize, usize),
    walls: Vec<Coord>,
}

type Position = usize;

#[derive(Debug, Deserialize, Serialize)]
struct Coord(usize, usize);

impl Coord {
    fn to_pos(&self, width: usize) -> usize {
        self.1 * width + self.0
    }

    fn from_pos(pos: Position, width: usize) -> Self {
        let x = pos % width;
        let y = pos / width;
        Coord(x, y)
    }
}

impl fmt::Display for Coord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let row = self.0 + 1;
        let column = char::from_u32(self.1 as u32 + 65u32).unwrap();
        write!(f, "{}{}", column, row)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum MazeCellKind {
    Wall,
    Empty,
    Entry,
    Exit,
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

        let c = parts[0].chars().next();
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

        let row = parts[1].parse::<usize>();
        if row.is_err() {
            return Err(Error {
                error: format!("Malformed cell: {}", row.unwrap_err()),
            });
        }
        let row = row.unwrap();
        if row == 0 {
            return Err(Error {
                error: String::from("Malformed cell: should start at 1"),
            });
        }

        let column = char_bytes[0] as usize - 65;

        Ok(Coord(row - 1, column))
    }
}

impl TryFrom<&web::Json<CreateMazeHttpRequest>> for CreateMaze {
    type Error = Error;

    fn try_from(value: &web::Json<CreateMazeHttpRequest>) -> Result<Self, Self::Error> {
        let coords: Vec<&str> = value.grid_size.split('x').collect();
        if coords.len() != 2 {
            return Err(Error {
                error: String::from("Malformed grid size"),
            });
        }

        let grid_size_width = coords[0].parse::<usize>();
        if grid_size_width.is_err() {
            return Err(Error {
                error: format!("Malformed grid size: {}", grid_size_width.unwrap_err()),
            });
        }
        let grid_size_height = coords[1].parse::<usize>();
        if grid_size_height.is_err() {
            return Err(Error {
                error: format!("Malformed grid size: {}", grid_size_height.unwrap_err()),
            });
        }

        let entrance = value.entrance.as_str().try_into()?;

        let mut walls = Vec::with_capacity(value.walls.len());
        for wall in value.walls.as_slice() {
            walls.push(wall.as_str().try_into()?);
        }

        Ok(CreateMaze {
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

struct MazePath {
    parents: Vec<Option<Position>>,
    leaf: Position,
}

// BFS traversal
fn shortest_path(
    maze: &[MazeCellKind],
    entrance_pos: Position,
    width: usize,
    height: usize,
) -> Option<MazePath> {
    let mut explored = Vec::with_capacity(width * height);
    explored.resize(explored.capacity(), false);

    let mut parents: Vec<Option<Position>> = Vec::with_capacity(width * height);
    parents.resize(parents.capacity(), None);

    let mut work: Vec<Position> = Vec::with_capacity(10);
    work.push(entrance_pos);

    explored[entrance_pos] = true;

    while let Some(work_pos) = work.pop() {
        if maze[work_pos] == MazeCellKind::Exit {
            return Some(MazePath {
                parents,
                leaf: work_pos,
            });
        }

        let work_coord = Coord::from_pos(work_pos, width);
        let adjacents: [(isize, isize); 4] = [
            (work_coord.0 as isize + 1, work_coord.1 as isize),
            (work_coord.0 as isize - 1, work_coord.1 as isize),
            (work_coord.0 as isize, work_coord.1 as isize + 1),
            (work_coord.0 as isize, work_coord.1 as isize - 1),
        ];

        for adjacent in adjacents {
            // Out of bounds
            if adjacent.0 < 0
                || adjacent.1 < 0
                || adjacent.0 as usize >= width
                || adjacent.1 as usize >= height
            {
                continue;
            }

            let adjacent_pos = Coord(adjacent.0 as usize, adjacent.1 as usize).to_pos(width);
            let kind = maze[adjacent_pos];

            // Do not go through walls
            if kind == MazeCellKind::Wall {
                continue;
            }

            if !explored[adjacent_pos] {
                explored[adjacent_pos] = true;
                parents[adjacent_pos] = Some(work_pos);
                work.push(adjacent_pos);
            }
        }
    }

    None
}

fn make_maze(create_maze: &CreateMaze) -> Vec<MazeCellKind> {
    let width = create_maze.grid_size.0;
    let height = create_maze.grid_size.1;
    let mut maze: Vec<MazeCellKind> = Vec::with_capacity(width * height);
    maze.resize(maze.capacity(), MazeCellKind::Empty);

    for cell in &create_maze.walls {
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

    let entrance_pos = create_maze.entrance.to_pos(width);
    maze[entrance_pos] = MazeCellKind::Entry;

    maze
}

fn draw_maze(maze: &[MazeCellKind], path: &[Position], width: usize, height: usize) {
    for y in 0..height {
        for x in 0..height {
            let pos = Coord(x, y).to_pos(width);
            match path.iter().find(|p| **p == pos) {
                Some(p) if *p == pos => {
                    print!("*");
                }
                _ => match maze[pos] {
                    MazeCellKind::Wall => print!("x"),
                    MazeCellKind::Empty => print!("."),
                    MazeCellKind::Exit => print!("o"),
                    MazeCellKind::Entry => print!("e"),
                },
            }
        }
        println!();
    }
}

fn create_maze_table_in_db(conn: &Connection) {
    conn.execute("CREATE TABLE IF NOT EXISTS mazes(maze BLOB NOT NULL)", [])
        .expect("Failed to create maze table");
}

fn create_maze_in_db(conn: &Connection, create_maze: &CreateMaze) -> Result<usize, Error> {
    let blob = match serde_json::to_string(&create_maze) {
        Ok(blob) => blob,
        Err(err) => {
            return Err(Error {
                error: format!("Failed to serialize maze to JSON: {}", err),
            })
        }
    };

    match conn.query_row(
        "INSERT INTO mazes VALUES (?) RETURNING rowid",
        [&blob],
        |row| row.get(0),
    ) {
        Ok(res) => Ok(res),
        Err(err) => Err(Error {
            error: format!("Failed to save maze in database: {}", err),
        }),
    }
}

async fn create_maze(req: web::Json<CreateMazeHttpRequest>) -> HttpResponse {
    let create_maze: CreateMaze = match (&req).try_into() {
        Err(err) => {
            return HttpResponse::BadGateway().json(err);
        }
        Ok(v) => v,
    };

    let conn = match Connection::open("maze") {
        Ok(conn) => conn,
        Err(err) => {
            return HttpResponse::BadGateway().json(Error {
                error: err.to_string(),
            })
        }
    };

    match create_maze_in_db(&conn, &create_maze) {
        Ok(id) => HttpResponse::Ok().json(json!({
            "id": id,
            "maze": create_maze,
        })),
        Err(err) => HttpResponse::BadGateway().json(err),
    }
}

fn get_maze_from_db(conn: &Connection, id: usize) -> Result<CreateMaze, Error> {
    let blob: String = match conn.query_row(
        "SELECT maze FROM mazes WHERE rowid = ? LIMIT 1",
        [id],
        |row| row.get(0),
    ) {
        Ok(blob) => blob,
        Err(err) => {
            return Err(Error {
                error: format!("Failed to read maze from database: {}", err),
            });
        }
    };

    serde_json::from_str(&blob).map_err(|err| Error {
        error: format!("Failed to deserialize maze from JSON: {}", err),
    })
}

fn collect_path(path: &MazePath) -> Vec<Position> {
    let mut path_pos = Vec::with_capacity(10);
    path_pos.push(path.leaf);

    let mut current_pos = path.leaf;
    while let Some(parent_pos) = path.parents[current_pos] {
        path_pos.push(parent_pos);
        current_pos = parent_pos;
    }

    path_pos.reverse();
    path_pos
}

async fn solve_maze(path: web::Path<usize>) -> HttpResponse {
    let maze_id: usize = path.into_inner();

    let conn = match Connection::open("maze") {
        Ok(conn) => conn,
        Err(err) => {
            return HttpResponse::BadGateway().json(Error {
                error: err.to_string(),
            })
        }
    };

    let create_maze = match get_maze_from_db(&conn, maze_id) {
        Err(err) => {
            return HttpResponse::BadGateway().json(err);
        }
        Ok(crate_maze) => crate_maze,
    };
    let maze = make_maze(&create_maze);
    let width = create_maze.grid_size.0;
    let height = create_maze.grid_size.1;

    let entrance_pos = Coord::to_pos(&create_maze.entrance, width);
    let path = match shortest_path(&maze, entrance_pos, width, height) {
        None => {
            return HttpResponse::BadRequest().json(Error {
                error: String::from("No path found, invalid maze"),
            });
        }
        Some(path) => path,
    };

    let path = collect_path(&path);
    draw_maze(&maze, &path, width, height);

    let human_readable_path = path
        .iter()
        .map(|pos| format!("{}", Coord::from_pos(*pos, width)))
        .collect::<Vec<String>>();
    HttpResponse::Ok().json(human_readable_path)
}

#[actix_web::main] // or #[tokio::main]
async fn main() -> std::io::Result<()> {
    env_logger::init();

    let conn = Connection::open("maze").expect("Failed to open db connection");
    create_maze_table_in_db(&conn);

    HttpServer::new(|| {
        App::new()
            // enable logger
            .wrap(middleware::Logger::default())
            .app_data(web::JsonConfig::default().limit(4096)) // <- limit size of the payload (global configuration)
            .service(web::resource("/maze").route(web::post().to(create_maze)))
            .service(web::resource("/maze/{id}/solution").route(web::get().to(solve_maze)))
    })
    .bind(("127.0.0.1", 8080))?
    .run()
    .await
}
