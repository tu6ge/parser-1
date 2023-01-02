default:
  @just --list

# build the library
build:
    cargo build

# regenerate test snapshots
snapshot:
    cargo run --bin pxp-parser-snapshot

# regenerate schema
schema:
    rm schema.json
    cargo run --bin pxp-parser-schema >> schema.json

# detect linting problems.
lint:
    cargo fmt --all -- --check
    cargo clippy

# fix linting problems.
fix:
    cargo fmt
    cargo clippy --fix --allow-dirty --allow-staged
    cargo fix --allow-dirty --allow-staged

# dump AST for the given file.
dump file *args:
    cargo run -r --bin pxp-parser-rs -- {{file}} {{args}}

# run all integration tests, except third-party.
test filter='--all':
    cargo test -r {{filter}}
