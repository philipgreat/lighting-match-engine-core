#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
ISO_ROOT="$ROOT_DIR/target/grub-iso"
KERNEL_ELF="$ROOT_DIR/target/x86_64-unknown-none/release/kernel"
ISO_PATH="$ROOT_DIR/target/lighting-match-engine-core.iso"

bash "$ROOT_DIR/scripts/build-kernel.sh"

rm -rf "$ISO_ROOT"
mkdir -p "$ISO_ROOT/boot/grub"
cp "$KERNEL_ELF" "$ISO_ROOT/boot/kernel.elf"
cp "$ROOT_DIR/grub/grub.cfg" "$ISO_ROOT/boot/grub/grub.cfg"

if command -v grub-mkrescue >/dev/null 2>&1; then
  grub-mkrescue -o "$ISO_PATH" "$ISO_ROOT"
else
  echo "grub-mkrescue is not installed; kernel ELF is ready at:"
  echo "  $KERNEL_ELF"
  echo "Install GRUB tools, then rerun scripts/build-grub-iso.sh to create a bootable ISO."
fi
