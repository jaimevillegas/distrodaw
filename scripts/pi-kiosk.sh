#!/usr/bin/env bash
# Prepara la Pi para arrancar directamente en DistroDAW con color de 24 bits:
# cage (compositor Wayland mínimo) + foot (terminal gráfico) en lugar de la
# consola del kernel, que solo tiene 16 colores.
#
# Uso (desde la carpeta donde está `distrodaw`, en la Pi):
#   ./pi-kiosk.sh install     instala cage, foot y las fuentes (pide sudo)
#   ./pi-kiosk.sh colortest   comprueba que el terminal muestra 24 bits
#   ./pi-kiosk.sh try         arranca el kiosco una vez (EN LA CONSOLA, no por SSH)
#   ./pi-kiosk.sh tune        permiso de tiempo real para el audio y CPU a máxima frecuencia (pide sudo)
#   ./pi-kiosk.sh untune      deshace `tune`
#   ./pi-kiosk.sh enable      arranque automático en tty1 (pide sudo)
#   ./pi-kiosk.sh disable     deshace `enable`
#   ./pi-kiosk.sh status      muestra qué está instalado y activado
set -euo pipefail

here="$(cd "$(dirname "$0")" && pwd)"
user="${SUDO_USER:-$(id -un)}"
home="$(getent passwd "$user" | cut -d: -f6)"
# Las rutas del sistema se pueden redirigir (para probar el script sin tocar /etc).
# Un ~/.bash_profile nuevo taparía ~/.profile (bash lee solo uno de los dos, y .profile
# carga .bashrc y el PATH), así que el bloque va en el que ya esté en uso.
if [ -f "$home/.bash_profile" ]; then default_profile="$home/.bash_profile"; else default_profile="$home/.profile"; fi
profile="${KIOSK_PROFILE:-$default_profile}"
dropin_dir="${KIOSK_DROPIN_DIR:-/etc/systemd/system/getty@tty1.service.d}"
dropin="$dropin_dir/distrodaw-autologin.conf"
limits_file="${KIOSK_LIMITS_DIR:-/etc/security/limits.d}/95-distrodaw-audio.conf"
governor_file="${KIOSK_TMPFILES_DIR:-/etc/tmpfiles.d}/distrodaw-governor.conf"
begin="# >>> distrodaw kiosk >>>"
end="# <<< distrodaw kiosk <<<"
packages=(cage foot fonts-jetbrains-mono fonts-dejavu-core)

as_root() {
    if [ "$(id -u)" -eq 0 ] || [ -n "${KIOSK_NO_SUDO:-}" ]; then "$@"; else sudo "$@"; fi
}

has_block() { [ -f "$profile" ] && grep -qF "$begin" "$profile"; }

cmd_install() {
    echo "Instalando: ${packages[*]}"
    as_root apt-get install -y "${packages[@]}"
    if [ ! -f "$here/kiosk.conf" ]; then
        cp "$here/kiosk.conf.example" "$here/kiosk.conf"
        [ "$(id -u)" -eq 0 ] && chown "$user": "$here/kiosk.conf"
        echo "Creado $here/kiosk.conf (edítalo para elegir tema, fuente y dispositivo)."
    fi
    echo "Listo. Siguiente paso, en la consola de la Pi: ./pi-kiosk.sh try"
}

cmd_colortest() {
    echo "TERM=${TERM:-?}  COLORTERM=${COLORTERM:-(vacío)}"
    echo "Degradados de 24 bits (deben verse suaves, sin escalones ni franjas):"
    for i in $(seq 0 3 255); do printf '\e[48;2;%d;%d;%dm \e[0m' "$i" "$((255 - i))" 128; done
    echo
    for i in $(seq 0 3 255); do printf '\e[48;2;%d;%d;%dm \e[0m' 255 "$i" "$((255 - i))"; done
    echo
    if [ -f "$here/themes/dracula.toml" ]; then
        echo "Muestras de Dracula:"
        for c in bd93f9 50fa7b f1fa8c ffb86c ff5555 6272a4; do
            printf '\e[48;2;%d;%d;%dm      \e[0m' "0x${c:0:2}" "0x${c:2:2}" "0x${c:4:2}"
        done
        echo
    fi
}

cmd_try() {
    if [ -n "${SSH_CONNECTION:-}" ] || [ "$(tty 2>/dev/null || true)" != /dev/tty1 ]; then
        echo "Esto debe ejecutarse en la consola de la Pi (tty1, con monitor y teclado), no por SSH." >&2
        exit 1
    fi
    command -v cage >/dev/null || { echo "Falta cage: ejecuta ./pi-kiosk.sh install" >&2; exit 1; }
    exec "$here/kiosk-session.sh"
}

cmd_tune() {
    echo "Esto cambia dos cosas del sistema (reversibles con ./pi-kiosk.sh untune):"
    echo "  1. $limits_file"
    echo "     deja que el grupo «audio» use prioridad de tiempo real (SCHED_FIFO) y memoria bloqueada."
    echo "     Así el hilo de audio no espera a la interfaz, al terminal ni al compositor."
    echo "  2. $governor_file"
    echo "     fija el gobernador de CPU en «performance» en cada arranque, para que la frecuencia"
    echo "     no suba y baje mientras tocas (con «ondemand» sube con retraso al empezar una nota)."
    echo "     Ojo: consume algo más; si la fuente de alimentación es justa, puede aumentar la"
    echo "     subtensión. Comprueba antes: vcgencmd get_throttled  (0x0 = sin problemas)."
    read -r -p "¿Continuar? [s/N] " answer
    [ "$answer" = s ] || [ "$answer" = S ] || { echo "Cancelado."; exit 0; }

    as_root mkdir -p "$(dirname "$limits_file")" "$(dirname "$governor_file")"
    as_root tee "$limits_file" >/dev/null <<EOT
# DistroDAW: tiempo real para el audio (lo escribe pi-kiosk.sh tune)
@audio - rtprio 95
@audio - memlock unlimited
EOT
    as_root tee "$governor_file" >/dev/null <<EOT
# DistroDAW: CPU a máxima frecuencia (lo escribe pi-kiosk.sh tune)
w /sys/devices/system/cpu/cpufreq/policy0/scaling_governor - - - - performance
EOT
    as_root systemd-tmpfiles --create "$governor_file" || echo "(no se pudo aplicar el gobernador ahora; se aplicará al reiniciar)"
    if ! id -nG "$user" | tr ' ' '\n' | grep -qx audio; then
        echo "AVISO: $user no está en el grupo «audio». Añádelo con: sudo usermod -aG audio $user"
    fi
    echo "Hecho. Cierra la sesión y vuelve a entrar (o reinicia) para que los límites se apliquen."
}

cmd_untune() {
    as_root rm -f "$limits_file" "$governor_file"
    echo "Deshecho. El gobernador de CPU vuelve a «ondemand» al reiniciar."
}

cmd_enable() {
    command -v cage >/dev/null || { echo "Falta cage: ejecuta ./pi-kiosk.sh install" >&2; exit 1; }
    echo "Esto activa el arranque automático de DistroDAW en la pantalla (tty1):"
    echo "  - autologin de $user en tty1 ($dropin)"
    echo "  - un bloque en $profile que lanza el kiosco"
    echo "SSH y las demás consolas no se ven afectadas. Para entrar a la consola normal,"
    echo "por SSH: touch $here/NOKIOSK  (y reiniciar)."
    read -r -p "¿Continuar? [s/N] " answer
    [ "$answer" = s ] || [ "$answer" = S ] || { echo "Cancelado."; exit 0; }

    as_root mkdir -p "$dropin_dir"
    as_root tee "$dropin" >/dev/null <<EOT
[Service]
ExecStart=
ExecStart=-/sbin/agetty --autologin $user --noclear %I \$TERM
EOT
    as_root systemctl daemon-reload

    if ! has_block; then
        cat >> "$profile" <<EOT
$begin
# Lanza DistroDAW en la pantalla (tty1). No afecta a SSH. Para saltarlo: touch $here/NOKIOSK
if [ -z "\${SSH_CONNECTION:-}" ] && [ -z "\${WAYLAND_DISPLAY:-}" ] && [ "\$(tty)" = /dev/tty1 ] && [ ! -e "$here/NOKIOSK" ]; then
    "$here/kiosk-session.sh" || sleep 5
    exit
fi
$end
EOT
        [ "$(id -u)" -eq 0 ] && chown "$user": "$profile"
    fi
    echo "Activado. Reinicia para probarlo: sudo reboot"
}

cmd_disable() {
    as_root rm -f "$dropin"
    as_root rmdir --ignore-fail-on-non-empty "$dropin_dir" 2>/dev/null || true
    as_root systemctl daemon-reload
    if has_block; then
        sed -i "/$(printf '%s' "$begin" | sed 's/[][\.*^$/]/\\&/g')/,/$(printf '%s' "$end" | sed 's/[][\.*^$/]/\\&/g')/d" "$profile"
    fi
    echo "Desactivado. Al reiniciar volverás a la consola normal."
}

cmd_status() {
    for p in "${packages[@]}"; do
        if dpkg -s "$p" >/dev/null 2>&1; then echo "paquete $p: instalado"; else echo "paquete $p: NO instalado"; fi
    done
    if [ -f "$dropin" ]; then echo "autologin tty1: activo"; else echo "autologin tty1: no"; fi
    if has_block; then echo "bloque en $profile: activo"; else echo "bloque en $profile: no"; fi
    if [ -f "$limits_file" ]; then echo "tiempo real (limits.d): configurado"; else echo "tiempo real (limits.d): NO configurado (ver tune)"; fi
    echo "límite de prioridad de tiempo real de esta sesión: $(ulimit -r) (0 = sin permiso)"
    if [ -f "$governor_file" ]; then echo "gobernador al arrancar: performance (configurado)"; else echo "gobernador al arrancar: el de por defecto"; fi
    gov="/sys/devices/system/cpu/cpu0/cpufreq/scaling_governor"
    if [ -r "$gov" ]; then echo "gobernador actual: $(cat "$gov")"; fi
    if [ -e "$here/NOKIOSK" ]; then echo "NOKIOSK: presente (el kiosco no arrancará)"; fi
    if [ -f "$here/kiosk.conf" ]; then echo "kiosk.conf: presente"; else echo "kiosk.conf: falta (lo crea install)"; fi
}

case "${1:-}" in
    install) cmd_install ;;
    colortest) cmd_colortest ;;
    try) cmd_try ;;
    tune) cmd_tune ;;
    untune) cmd_untune ;;
    enable) cmd_enable ;;
    disable) cmd_disable ;;
    status) cmd_status ;;
    *) sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 1 ;;
esac
