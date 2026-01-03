#!/bin/bash
# Run E2E tests. Pass test name filter as argument.
# Usage: ./scripts/e2e.sh [test_filter]
set -e
cargo test --test e2e -- --test-threads=1 "$@"
