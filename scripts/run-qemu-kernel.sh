#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ISO_PATH="$ROOT_DIR/target/lighting-match-engine-core.iso"

bash "$ROOT_DIR/scripts/build-grub-iso.sh"

if [[ ! -f "$ISO_PATH" ]]; then
  echo "bootable ISO was not created"
  echo "Install grub-mkrescue, then rerun this script."
  exit 1
fi

exec qemu-system-x86_64 \
  -cdrom "$ISO_PATH" \
  -serial stdio \
  -display none \
  -no-reboot \
  -no-shutdown
