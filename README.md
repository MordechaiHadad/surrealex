# Surrealex

Dead simple SurrealDB query generator.

A Rust library for building SurrealQL queries with a fluent, type-safe API.

## 🌟 Features

- Fluent builder API using `QueryBuilder`
- Complex WHERE conditions via the `Condition` enum
- Supports `SELECT`, `FROM`, `WHERE`, `FETCH`, `ORDER BY`, `LIMIT`, and `START`
- No external dependencies

## 📦 Requirements

- Rust 1.65 or later

## 🔧 Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
surrealex = "0.1.0"
```

Or from Git:

```toml
[dependencies]
surrealex = { git = "https://github.com/MordechaiHadad/surrealex" }
```

Run:

```bash
cargo build
```

## ❓ Usage

Basic example:

```rust
use surrealex::{QueryBuilder, Condition};

let query = QueryBuilder::new()
    .select("id, name")
    .from("user")
    .add_where("age > 18")
    .order_by("age DESC")
    .limit(10)
    .build()
    .unwrap();

assert_eq!(query, 
    "SELECT id, name FROM user WHERE age > 18 ORDER BY age DESC LIMIT 10"
);
```

Complex conditions:

```rust
let cond = Condition::And(vec![
    Condition::Simple("age > 18".into()),
    Condition::Or(vec![
        Condition::Simple("status = 'active'".into()),
        Condition::Simple("status = 'pending'".into()),
    ]),
]);

let query = QueryBuilder::new()
    .from("user")
    .add_condition(cond)
    .build()
    .unwrap();

assert_eq!(query,
    "SELECT * FROM user WHERE (age > 18 AND (status = 'active' OR status = 'pending'))"
);
```

## 🚀 API

See [`src/lib.rs`](src/lib.rs) for full documentation.

## 🔗 Graph Traversal

Perform two-step graph expansions with explicit directions and optional alias using `graph_traverse`:

```rust
use surrealex::{QueryBuilder, Direction, GraphExpandParams};

let sql = QueryBuilder::new()
    .from("user")
    .graph_traverse(GraphExpandParams {
        from: (Direction::Out, "friends".into()),
        to:   (Direction::In,  "posts".into()),
        alias: Some("friend_posts".into()),
    })
    .build()
    .unwrap();

assert_eq!(sql,
    "SELECT * FROM user ->friends->posts.* AS friend_posts"
);
```