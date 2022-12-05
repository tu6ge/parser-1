default:
  @just --list

# build the library
build:
    cargo build

# regenerate test snapshots
snapshot:
    cargo run --bin snapshot

# detect linting problems.
lint:
    cargo fmt --all -- --check
    cargo clippy

# fix linting problems.
fix:
    cargo fmt
    cargo clippy --fix --allow-dirty --allow-staged

# dump AST for the given file.
dump file: build
    cargo run --bin php-parser-rs -- {{file}}

# run all integration tests, except third-party.
test filter='': build
    cargo test --all {{filter}} -- --skip third_party

# run integration tests for third-party libraries.
test-third-party: build
    cargo test third_party
