# Maze

## Quickstart

```
$ RUST_LOG=debug cargo run --release

$ curl -H 'Content-Type: application/json' -d @./req.json localhost:8080/maze
{"id":6,"maze":{"entrance":[0,0],"grid_size":[8,8],"walls":[[0,2],[0,6],[1,0],[1,2],[1,4],[1,6],[2,2],[2,4],[3,1],[3,2],[3,4],[3,5],[3,6],[4,1],[4,4],[5,3],[5,4],[5,6],[5,7],[6,1],[6,3],[6,6],[7,1]]}}

$ curl http://localhost:8080/maze/5/solution
["A1","B1","B2","B3","A3","A4","A5","A6","B6","C6","C5","D5","D4","D3","D2","D1","E1","F1","F2","F3","G3","H3","H2","H1"]

```

## Implemented

- Endpoint to create a maze
- Store it in the database (sqlite)
- Endpoint to solve a maze

The rest is not implemented out of time constraints.
