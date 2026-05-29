#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
crate_dir="$(cd -- "${script_dir}/.." && pwd)"
cd "$crate_dir"

cargo fmt --check
cargo test --locked --lib --all-features
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo rustdoc --locked --all-features -- -D missing_docs -D rustdoc::broken_intra_doc_links
cargo package --locked --allow-dirty
