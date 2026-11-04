#!/usr/bin/env bash
# Sesión del kiosco: cage (compositor mínimo) + foot (terminal de 24 bits)
# ejecutando distrodaw a pantalla completa. Debe lanzarse desde la consola
# física (tty1), no por SSH. Configuración en `kiosk.conf`.
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"

THEME=dracula
FONT="JetBrains Mono"
FONT_SIZE=13
AUDIO_ARGS=""
TUI_ARGS=""
# shellcheck disable=SC1091
[ -f "$here/kiosk.conf" ] && . "$here/kiosk.conf"

theme_file="$here/themes/$THEME.toml"
if [ ! -f "$theme_file" ]; then
    echo "No existe el tema $theme_file" >&2
    exit 1
fi

# Fondo y texto de foot = los del tema, para que no se note el borde de la ventana.
hex() {
    sed -n "s/^$1[[:space:]]*=[[:space:]]*\"#\([0-9a-fA-F]\{6\}\)\".*/\1/p" "$theme_file" | head -n1
}
bg="$(hex background)"
fg="$(hex foreground)"

config="$(mktemp --suffix=.ini)"
trap 'rm -f "$config"' EXIT
cat > "$config" <<EOT
[main]
font=$FONT:size=$FONT_SIZE,DejaVu Sans Mono:size=$FONT_SIZE
pad=14x10

[colors]
background=${bg:-282a36}
foreground=${fg:-f8f8f2}
EOT

cd "$here"
# AUDIO_ARGS y TUI_ARGS son listas de argumentos: deben expandirse sin comillas.
# shellcheck disable=SC2086
cage -s -- foot --config="$config" "$here/distrodaw" $AUDIO_ARGS tui --theme "$THEME" --color truecolor $TUI_ARGS
