#!/usr/bin/env bash
# Compila para Raspberry Pi 2 (ARMv7) y copia el binario y el banco de pruebas.
# Uso: scripts/deploy-pi.sh usuario@host [directorio-remoto]
set -euo pipefail

target="armv7-unknown-linux-gnueabihf"
host="${1:?uso: $0 usuario@host [directorio-remoto]}"
dest="${2:-distrodaw}"

cd "$(dirname "$0")/.."
cross build --release --target "$target" -p distrodaw

ssh "$host" "mkdir -p '$dest'"
scp "target/$target/release/distrodaw" scripts/bench-pi.sh "$host:$dest/"
echo "Copiado a $host:$dest. En la Pi: cd $dest && ./bench-pi.sh"
