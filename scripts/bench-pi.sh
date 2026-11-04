#!/usr/bin/env bash
# Mide carga de CPU y plazos perdidos para varias combinaciones de buffer y voces.
# Se ejecuta en la Pi, junto al binario `distrodaw`.
# Uso: ./bench-pi.sh [fragmento-del-dispositivo] [segundos-por-prueba]
set -euo pipefail

device="${1:-}"
seconds="${2:-5}"
rate=48000
out="bench-$(date +%Y%m%d-%H%M%S).txt"
args=()
[ -n "$device" ] && args+=(--device "$device")

cd "$(dirname "$0")"
{
  echo "# $(uname -a)"
  echo "# gobernador: $(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor 2>/dev/null || echo desconocido)"
  echo "# frecuencia: $(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq 2>/dev/null || echo desconocida) kHz"
  for buffer in 64 128 256; do
    for voices in 1 8 16 32 64; do
      echo "## buffer=$buffer voces=$voices"
      ./distrodaw "${args[@]}" --rate "$rate" --buffer "$buffer" --voices "$voices" --seconds "$seconds" \
        2>&1 | grep -E 'RESUMEN|error|Error' || echo "FALLO al abrir el stream"
    done
  done
} | tee "$out"
echo "Resultados guardados en $out"
