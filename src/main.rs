use actix_web::{middleware, web, App, HttpResponse, HttpServer};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;
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

#[derive(Debug, Copy, Clone)]
struct Node {
    maze_pos: Position,
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

// BFS traversal
fn shortest_path(
    maze: &[MazeCellKind],
    entrance_pos: Position,
    width: usize,
    height: usize,
) -> Option<(Vec<Node>, Node)> {
    let mut explored = Vec::with_capacity(width * height);
    explored.resize(explored.capacity(), false);

    let root_node = Node {
        maze_pos: entrance_pos,
        parent_idx: None,
    };
    let mut nodes: Vec<Node> = Vec::with_capacity(width * height);
    nodes.push(root_node);

    let mut work: Vec<usize> = Vec::new();
    work.push(nodes.len() - 1);

    explored[entrance_pos] = true;

    while !work.is_empty() {
        let work_node_idx = work.pop().unwrap();
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
            let kind = maze[adjacent_cell_pos];

            if kind == MazeCellKind::Wall {
                continue;
            }

            if !explored[adjacent_cell_pos] {
                explored[adjacent_cell_pos] = true;
                let adjacent_cell_node = Node {
                    maze_pos: adjacent_cell_pos,
                    parent_idx: Some(work_node_idx),
                };
                nodes.push(adjacent_cell_node);
                work.push(nodes.len() - 1);
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

fn collect_path(nodes: &[Node], exit: &Node, width: usize) -> Vec<String> {
    let mut path = Vec::new();
    let mut current_node = *exit;

    loop {
        let coord = Coord::from_pos(current_node.maze_pos, width);
        path.push(format!("{}", coord));

        if let Some(parent_idx) = current_node.parent_idx {
            current_node = nodes[parent_idx];
        } else {
            break;
        }
    }

    path.reverse();
    path
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
    let (nodes, exit) = match shortest_path(&maze, entrance_pos, width, height) {
        None => {
            return HttpResponse::BadRequest().json(Error {
                error: String::from("No path found, invalid maze"),
            });
        }
        Some(path) => path,
    };

    let path = collect_path(&nodes, &exit, width);

    // let sorted = tsort(&maze, width, height);
    // println!("{:?}", sorted);
    let tree = make_tree(&maze, entrance_pos, width, height);
    println!("{:?}", tree);

    HttpResponse::Ok().json(path)
}

#[derive(Debug)]
struct N {
    pos: Position,
    kind: MazeCellKind,
    children: Vec<N>,
}

fn make_tree(maze: &[MazeCellKind], entrance_pos: Position, width: usize, height: usize) -> N {
    let mut root = N {
        pos: entrance_pos,
        kind: maze[entrance_pos],
        children: Vec::new(),
    };
    let mut visited = HashSet::new();
    visited.insert(entrance_pos);
    make_tree_helper(maze, &mut root, width, height, &mut visited);

    root
}

fn make_tree_helper(
    maze: &[MazeCellKind],
    parent: &mut N,
    width: usize,
    height: usize,
    visited_pos: &mut HashSet<usize>,
) {
    let coord = Coord::from_pos(parent.pos, width);
    let adjacent_cells: [(isize, isize); 4] = [
        (coord.0 as isize + 1, coord.1 as isize),
        (coord.0 as isize - 1, coord.1 as isize),
        (coord.0 as isize, coord.1 as isize + 1),
        (coord.0 as isize, coord.1 as isize - 1),
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
        let kind = maze[adjacent_cell_pos];

        if kind == MazeCellKind::Wall {
            continue;
        }

        if visited_pos.contains(&adjacent_cell_pos) {
            continue;
        }

        visited_pos.insert(adjacent_cell_pos);
        let mut adjacent_node = N {
            pos: adjacent_cell_pos,
            kind,
            children: Vec::new(),
        };

        make_tree_helper(
            maze,
            &mut adjacent_node,
            width,
            height,
            visited_pos,
        );
        parent.children.push(adjacent_node);
    }
}

// fn tsort(
//     maze: &[MazeCellKind],
//     entrance_pos: Position,
//     width: usize,
//     height: usize,
// ) -> Result<Vec<usize>, Error> {
//     let mut permanently_marked = Vec::with_capacity(width * height);
//     permanently_marked.resize(permanently_marked.capacity(), false);

//     let mut temporarily_marked = Vec::with_capacity(width * height);
//     temporarily_marked.resize(temporarily_marked.capacity(), false);

//     let mut nodes = maze
//         .iter()
//         .map(|k| {
//             if k == &MazeCellKind::Wall {
//                 None
//             } else {
//                 Some(())
//             }
//         })
//         .collect::<Vec<_>>();
//     let mut sorted_node_indices = Vec::with_capacity(width * height);

//     while let Some(unmarked_pos) = permanently_marked.iter().position(|m| *m == false) {
//         tsort_visit(
//             maze,
//             unmarked_pos,
//             &mut nodes,
//             &mut sorted_node_indices,
//             &mut permanently_marked,
//             &mut temporarily_marked,
//             width,
//             height,
//         )?;
//     }
//     Ok(sorted_node_indices)
// }

// fn tsort_visit(
//     maze: &[MazeCellKind],
//     pos: usize,
//     nodes: &mut Vec<Option<()>>,
//     sorted_node_indices: &mut Vec<usize>,
//     permanently_marked: &mut [bool],
//     temporarily_marked: &mut [bool],
//     width: usize,
//     height: usize,
// ) -> Result<(), Error> {
//     let node = nodes[pos];
//     if node.is_none() {
//         return Ok(());
//     }
//     let kind = maze[pos];

//     if kind == MazeCellKind::Wall {
//         return Ok(());
//     }

//     if permanently_marked[pos] {
//         return Ok(());
//     }

//     if temporarily_marked[pos] {
//         return Err(Error {
//             error: String::from("Graph has at least one cycle"),
//         });
//     }

//     temporarily_marked[pos] = true;

//     let coord = Coord::from_pos(pos, width);
//     let adjacent_cells: [(isize, isize); 4] = [
//         (coord.0 as isize + 1, coord.1 as isize),
//         (coord.0 as isize - 1, coord.1 as isize),
//         (coord.0 as isize, coord.1 as isize + 1),
//         (coord.0 as isize, coord.1 as isize - 1),
//     ];

//     for adjacent_cell in adjacent_cells {
//         // Out of bounds
//         if adjacent_cell.0 < 0
//             || adjacent_cell.1 < 0
//             || adjacent_cell.0 as usize >= width
//             || adjacent_cell.1 as usize >= height
//         {
//             continue;
//         }

//         let adjacent_cell_pos =
//             Coord(adjacent_cell.0 as usize, adjacent_cell.1 as usize).to_pos(width);
//         let kind = maze[adjacent_cell_pos];

//         if kind == MazeCellKind::Wall {
//             continue;
//         }

//         if permanently_marked[pos] || temporarily_marked[pos] {
//             continue;
//         }

//         tsort_visit(
//             maze,
//             pos,
//             nodes,
//             sorted_node_indices,
//             permanently_marked,
//             temporarily_marked,
//             width,
//             height,
//         )?;
//     }

//     temporarily_marked[pos] = false;
//     permanently_marked[pos] = true;

//     sorted_node_indices.push(pos);
//     Ok(())
// }

#[actix_web::main] // or #[tokio::main]
async fn main() -> std::io::Result<()> {
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
