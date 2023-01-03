# Maze

## Quickstart

```
$ RUST_LOG=debug cargo run --release
$ curl -H 'Content-Type: application/json' -d @./req.json localhost:8080/maze
$ curl http://localhost:8080/maze/5/solution
```

## Implemented

- Endpoint to create a maze
- Store it in the database (sqlite)
- Endpoint to solve a maze

The rest is not implemented out of time constraints.
