# Hito 3 — TUI

Objetivo (ver `SPEC.md`): pantallas Play y Synth con medidores y edición de
parámetros; entrada de teclado y joystick.

**Estado:** teclado listo y probado; **el joystick queda pendiente** hasta tener
uno para probarlo en la Pi (ver «Pendiente»).

## Uso

```sh
distrodaw                      # abre la interfaz (equivale a `distrodaw tui`)
distrodaw tui --midi Keystation
distrodaw --device hw:CARD=V2,DEV=0 --rate 48000 --buffer 128 tui
```

Sin `--midi` la interfaz arranca aunque no haya teclado MIDI (el puerto se
muestra como «sin teclado MIDI»); con `--midi` falla si no encuentra el puerto.
`distrodaw play` sigue siendo el modo de texto con una línea de estado.

### Pantallas

- **Play (F1):** patch, puerto MIDI, dispositivo de audio, voces activas (8
  casillas), últimas notas tocadas.
- **Synth (F2):** cadena OSC → AMP → OUT y los seis parámetros con barra y valor
  (waveform, attack, decay, sustain, release, volume). El seleccionado va en
  vídeo inverso con ▶.
- **Pie (siempre):** medidor de salida, voces, carga de CPU del callback y
  *xruns* (plazos perdidos + errores del stream).

Solo usa los 16 colores básicos y cabe en una consola de 80x25 (mínimo 44x14).

### Teclas

| Tecla | Acción |
|-------|--------|
| F1 / F2 | Pantalla Play / Synth |
| Tab | Pantalla siguiente |
| ↑ ↓ (k j) | Parámetro anterior / siguiente |
| ← → (h l) | Ajuste normal (2 % del rango) |
| Mayús + ← →, `,` `.` | Ajuste fino (0,4 %) |
| RePág / AvPág | Ajuste grueso (10 %) |
| q, Ctrl+C | Salir |

El ajuste fino también está en `,` y `.` porque la consola del kernel no
distingue Mayús + flechas. La forma de onda cambia de uno en uno. Los tiempos
(attack, decay, release) usan una curva cuadrática, igual que los CC MIDI.

## Cómo está hecho

- **`crates/tui`**: `UiState` y `Action` (sin terminal ni entrada, se prueban sin
  pantalla), `input` (teclas → acciones), `ui` (dibujo con `ratatui`), `format` y
  `terminal` (modo raw y pantalla alternativa, restaurados también ante un pánico).
- **Entrada unificada:** teclado, y más adelante joystick, producen las mismas
  `Action`; la interfaz no sabe de dónde vienen.
- **Audio → interfaz sin locks:** el hilo de audio publica pico, parámetros y
  voces en atómicos (`Shared`); la interfaz los lee 30 veces por segundo. Así los
  CC del teclado MIDI también se ven en la pantalla Synth.
- **Interfaz → audio:** los cambios de parámetro viajan como `Event::set_param`
  por una cola `rtrb` propia (otra distinta de la del teclado MIDI).
- **Sesión compartida:** `play` y `tui` usan la misma `Session` (audio, sinte,
  MIDI y colas). En la TUI no se escribe nada por la consola.
- **Carga de CPU:** ahora se calcula sobre el último intervalo, no desde el arranque.

## Pruebas

- [x] Tests: 24 en `tui` (acciones, edición de parámetros, teclas y dibujo contra
      un buffer de pantalla en memoria), 16 en la app, 35 en el motor.
- [x] PC y Pi 2 en un terminal virtual (pty) de 80x25 con teclas reales: cambiar de
      pantalla, elegir y ajustar `attack`, salir con `q` (código 0).
- [x] Notas y CC enviados por el puerto virtual «Midi Through»: las notas aparecen en
      Play (voces, medidor) y los CC 7 y 70 se reflejan en Synth.
- [x] Pi 2 con el Volt 2 (buffer 128, 48 kHz): carga de CPU ≈ 1 %, 0 xruns con la
      interfaz abierta.

- [x] **Consola del kernel por HDMI** en la Pi 2, con monitor y teclado USB (comprobado
      a mano por el autor): la interfaz funciona y los símbolos ▓ ░ ■ □ ▶ se ven bien.

## Pendiente

- [ ] **Joystick / gamepad (`gilrs`):** no hay ninguno para probar; se añadirá como
      otra fuente de `Action` sin tocar la interfaz.
- [ ] Probarla por **SSH** desde un terminal real (hasta ahora solo en un pty de prueba).
- [ ] Mensajes de `libasound` por `stderr`: salen al enumerar dispositivos al
      arrancar (antes de abrir la pantalla); no se han visto durante el uso.
- [ ] Pantallas Seq, Song y Files (hitos 4 y 5).
