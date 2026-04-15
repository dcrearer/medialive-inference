#!/usr/bin/env bash
set -euo pipefail

cd "$(dirname "$0")/lambda/event_logger"

# Build for ARM64 Lambda
cargo lambda build --release --arm64

echo "Build complete. Asset at: lambda/event_logger/target/lambda/bootstrap/"
