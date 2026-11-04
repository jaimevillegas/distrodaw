#!/usr/bin/env bash
# Compila para Raspberry Pi 2 (ARMv7) y copia el binario, el banco de pruebas, los patches, los temas y el kiosco.
# Uso: scripts/deploy-pi.sh usuario@host [directorio-remoto]
set -euo pipefail

target="armv7-unknown-linux-gnueabihf"
host="${1:?uso: $0 usuario@host [directorio-remoto]}"
dest="${2:-distrodaw}"

cd "$(dirname "$0")/.."
cross build --release --target "$target" -p distrodaw

ssh "$host" "mkdir -p '$dest'"
scp "target/$target/release/distrodaw" scripts/bench-pi.sh "$host:$dest/"
scp -r patches themes "$host:$dest/"
scp scripts/pi-kiosk.sh scripts/kiosk-session.sh scripts/kiosk.conf.example "$host:$dest/"
# Fuerza el volcado a la SD y comprueba que lo escrito es idéntico a lo enviado. Sin esto, si la
# Pi se reinicia justo después de copiar, los archivos pueden quedar con el tamaño correcto
# pero llenos de ceros.
{
    (cd "target/$target/release" && sha256sum distrodaw)
    (cd scripts && sha256sum bench-pi.sh pi-kiosk.sh kiosk-session.sh kiosk.conf.example)
    sha256sum patches/* themes/*
} | ssh "$host" "cd '$dest' && sync && sha256sum --quiet -c -"

echo "Copiado a $host:$dest (binario, bench-pi.sh, patches/, themes/ y el kiosco). En la Pi: cd $dest && ./bench-pi.sh"
