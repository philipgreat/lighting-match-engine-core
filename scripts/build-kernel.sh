#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cargo +nightly build \
  -Z build-std=core,alloc \
  -Z build-std-features=compiler-builtins-mem \
  --release \
  --target x86_64-unknown-none \
  --features baremetal \
  --config "target.x86_64-unknown-none.rustflags=[\"-C\",\"link-arg=-T$ROOT_DIR/linker.ld\"]" \
  --bin kernel
