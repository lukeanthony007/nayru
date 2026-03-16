set shell := ["bash", "-eu", "-o", "pipefail", "-c"]

default:
  @just --list

launch:
  cargo tauri dev

test:
  cargo test --workspace

build:
  cargo tauri build

fmt:
  cargo fmt --all

lint:
  cargo clippy --workspace --all-targets -- -D warnings

check:
  cargo check --workspace

clean:
  cargo clean

# Project-specific

dev-next:
  cd nayru-app && next dev -p 3002

launch-cli *ARGS:
  cargo run --bin nayru -- {{ARGS}}

serve:
  cargo run --bin nayru -- serve
