#!/bin/bash
# Run all tests with coverage.
# Report: target/llvm-cov/html/index.html
set -e
export RUSTUP_TOOLCHAIN=nightly-2026-01-01
export RUSTFLAGS='-C instrument-coverage -Zcoverage-options=branch --cfg=coverage --cfg=trybuild_no_target'
export LLVM_PROFILE_FILE="$PWD/target/dome-%p-%11m.profraw"
export CARGO_LLVM_COV=1
export CARGO_LLVM_COV_TARGET_DIR="$PWD/target"
cargo llvm-cov clean --workspace
cargo test --lib
# Can't spin up multiple dome servers at the same time
cargo test --test e2e -- --test-threads=1
cargo llvm-cov report --branch --html
