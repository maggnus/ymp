#!/bin/sh
set -eu
repository=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
cd "$repository"
npm ci --prefix ymp-bridges/claude --no-audit --no-fund
npm run build --prefix ymp-bridges/claude
cargo build --release --locked
install_dir="${YMP_BIN_DIR:-$HOME/.local/bin}"
mkdir -p "$install_dir"
if [ -e "$install_dir/ymp" ] && [ ! -L "$install_dir/ymp" ]; then
    backup_dir="${YMP_HOME:-$HOME/.ymp2}/backups"
    mkdir -p "$backup_dir"
    backup="$backup_dir/ymp-$(date -u +%Y%m%dT%H%M%SZ).previous"
    cp -p "$install_dir/ymp" "$backup"
    printf '%s\n' "Previous executable saved: $backup"
fi
ln -sfn "$repository/target/release/ymp" "$install_dir/ymp"
printf '%s\n' "Installed: $install_dir/ymp" "Run ymp doctor to inspect local providers."
