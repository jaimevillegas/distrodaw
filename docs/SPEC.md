# DistroDAW — Especificación del proyecto

> Estado: borrador v0.1. Documento vivo; se actualiza al cerrar cada fase.

## 1. Visión

DistroDAW es un DAW **solo MIDI**, de interfaz **tipo terminal**, que arranca
directamente sobre un kernel de Linux (encender la Raspberry Pi y que cargue el
DAW, sin escritorio). Se comporta más como un instrumento que como un programa:
se toca con teclado MIDI, se diseñan sintetizadores, se secuencia y se graba.

Es software libre (FOSS) bajo licencia **MIT**: se puede copiar, modificar y
usar para cualquier fin.

### Principios

1. **Instrumento primero:** arranque rápido, una acción por pantalla, todo con teclado y joystick.
2. **Ligero:** debe funcionar en una Raspberry Pi 2 (ARMv7, 1 GB RAM).
3. **Portable:** el mismo código compila para ARM (Pi) y x86_64 (PC).
4. **Hackeable:** los sintes y patches se editan como texto, sin recompilar.
5. **Tiempo real serio:** el hilo de audio nunca reserva memoria ni usa locks.

## 2. Alcance

### 2.1 Dentro del alcance (v1)

- Entrada de teclado MIDI (y otros controladores MIDI).
- Motor de sintetizadores polifónicos con parámetros controlables.
- Diseño de sintes mediante patches en archivos de texto.
- Secuenciador tipo tracker.
- Grabación y exportación a archivos `.mid`.
- Interfaz TUI (consola del kernel y SSH).
- Salida de audio por interfaz USB class compliant (Universal Audio Volt 2) o por el jack de la Pi.
- Entradas adicionales por USB: teclado de computador y joystick/gamepad.
- Compilación para Raspberry Pi 2 y para PC x86_64.
- Imagen de arranque directo al DAW.

### 2.2 Fuera del alcance (por ahora)

- Grabación o edición de audio (el DAW es solo MIDI).
- Plugins de terceros (VST, LV2, CLAP).
- Mezcla multipista avanzada, automatización compleja y efectos pesados.
- Piano roll y editor de grafo modular visual (candidatos a versiones posteriores).
- Pantalla Song / arreglo completo (fase posterior).
- GUI de escritorio (posible crate alternativo en el futuro).

## 3. Hardware objetivo

| Elemento | Detalle |
|----------|---------|
| Placa inicial | Raspberry Pi 2 (ARMv7, ~900 MHz, 1 GB, un solo bus USB 2.0 compartido) |
| Placa futura | Pi 3/4 si la Pi 2 se queda corta en polifonía o efectos |
| Audio | Universal Audio Volt 2 (class compliant, vía ALSA, sin drivers) o jack de 3.5 mm como alternativa |
| Controladores | Teclado MIDI, teclado USB, joystick/gamepad |
| Alimentación USB | Hub USB con alimentación propia recomendado (Volt 2 + varios dispositivos comparten un bus) |
| Desarrollo | PC x86_64 con Linux; pruebas finales en la Pi |

Expectativa de rendimiento en Pi 2 (**estimación a validar en el Hito 1**):
unas 8 a 16 voces simples; sintes con filtros pesados o reverb probablemente
requieran Pi 4.

## 4. Stack técnico

| Área | Elección |
|------|----------|
| Lenguaje | Rust (compila a ARMv7 y x86_64) |
| Audio | `cpal` (ALSA en Linux; WASAPI/CoreAudio en otros sistemas) |
| MIDI en vivo | `midir` |
| Archivos MIDI | `midly` |
| TUI | `ratatui` + `crossterm` |
| Teclado con note-off / joystick | `evdev` (bajo nivel) y `gilrs` (gamepads) |
| Cola entre hilos | `rtrb` (sin bloqueo) |
| DSP | Propio al inicio (osciladores, ADSR, filtro); `fundsp` como opción futura |
| Patches | TOML (u otro formato de texto simple) |
| Compilación cruzada | `cross`, target `armv7-unknown-linux-gnueabihf` |
| Imagen de arranque | Por decidir: Buildroot (arranque rápido, imagen mínima) o Raspberry Pi OS Lite con autologin |

## 5. Arquitectura

### 5.1 Workspace de Cargo

```
DistroDAW/
├── crates/
│   ├── engine/   # sintes, voces, secuenciador. Sin UI ni I/O.
│   ├── tui/      # interfaz de terminal
│   └── app/      # binario `distrodaw`: une audio, MIDI, entradas y TUI
├── docs/
├── Cross.toml    # configuración de compilación cruzada
└── LICENSE
```

Una futura GUI ligera sería otro crate que reemplaza a `tui` sin tocar el motor.

### 5.2 Hilos

- **Hilo de audio** (tiempo real): ejecuta el motor. Prohibido reservar memoria, usar locks o hacer I/O.
- **Hilo de UI/entradas:** TUI, MIDI, teclado, joystick. Se comunica con el motor por colas sin bloqueo.

### 5.3 Decisiones de arquitectura (fijadas desde el inicio)

1. **Parámetros con identidad.** Cada control (cutoff, attack, etc.) es un parámetro con ID, nombre, rango y valor. TUI, joystick, MIDI CC y archivos de patch controlan los mismos parámetros, lo que permite mapear cualquier control a cualquier parámetro.
2. **Eventos con marca de tiempo.** Todo lo que entra al motor (note on/off, cambio de parámetro) lleva su posición en frames dentro del buffer de audio. Es lo que mantiene al secuenciador en tiempo.
3. **Entrada unificada.** MIDI, teclado y joystick se convierten a un mismo tipo de evento interno en `app`; el motor no sabe de dónde vienen.
4. **Voz fija antes que grafo libre.** Una voz con 2 osciladores, filtro, 2 envolventes y 1 LFO, con todo configurable, cubre muchos sonidos. El grafo modular se difiere hasta que el motor funcione.
5. **Patches en texto.** Un sinte se describe en un archivo que el motor carga (idealmente en caliente), de modo que se edita en la Pi sin recompilar. Los módulos base se escriben en Rust.

## 6. Interfaz de usuario

TUI modal estilo tracker: pantallas completas que se cambian con teclas F, en
lugar de ventanas flotantes. Pensada para la consola del kernel (16 colores) y SSH.

| Pantalla | Tecla | Función |
|----------|-------|---------|
| Play | F1 | Medidores, voces activas, patch actual, últimas notas. Tocar en vivo. |
| Synth | F2 | Parámetros del patch (osc → filtro → env → out), editables con teclas o joystick. |
| Seq | F3 | Patrones tipo tracker: filas de pasos, columnas por pista. |
| Song | F4 | Orden de patrones y arreglo (fase posterior). |
| Files | F5 | Cargar y guardar patches, patrones y `.mid`. |

Boceto de la pantalla Synth:

```
┌ SYNTH: Pad01 ─────────────┐
│ OSC1 saw ─┐               │
│ OSC2 sqr ─┴▶ FILT ▶ AMP ▶ │
│           lp      adsr    │
│ cutoff ▓▓▓▓░░░░  800 Hz   │
│ reso   ▓▓░░░░░░  0.2      │
│ attack ▓░░░░░░░  10 ms    │
├───────────────────────────┤
│ L ▁▃▅▇  R ▁▃▅  voces 5/8  │
└ F1 Play F2 Synth F3 Seq ──┘
```

### Principios de interacción

- Todo se maneja con teclado y joystick, sin ratón.
- El parámetro seleccionado se mueve con flechas; una tecla modificadora da ajuste fino.
- Medidores y estado siempre visibles en la parte inferior.
- Paleta de pocos colores.
- Se respetan las convenciones de los trackers para entrada de notas, cursor, copiar bloques y cambio de pantalla.

### Entradas

| Dispositivo | Mecanismo | Notas |
|-------------|-----------|-------|
| Teclado MIDI | `midir` vía ALSA | Entrada principal para tocar. |
| Teclado USB (control de la TUI) | `crossterm` | La terminal no entrega "soltar tecla". |
| Teclado USB como piano | `evdev` (`/dev/input/event*`) | Permite note-off real. El usuario debe estar en el grupo `input`. |
| Joystick / gamepad | `gilrs` o `evdev` | Ejes y botones mapeables a parámetros (cutoff, pitch bend, modulación, cambio de patch). |

## 7. Fases e hitos

| Hito | Objetivo | Criterio de aceptación |
|------|----------|------------------------|
| 0 | Workspace y oscilador en PC | Una sinusoide de 440 Hz suena por la salida por defecto. **Hecho.** |
| 1 | Compilar a la Pi 2 y medir | Sinusoide por el Volt 2 en la Pi 2; latencia medida con buffer de 64 a 128 frames a 48 kHz; uso de CPU registrado. Define cuántas voces caben. |
| 2 | MIDI y sinte polifónico | Teclado MIDI → sinte de 8 voces con ADSR, controlado por parámetros con ID; eventos con marca de tiempo. Meta de latencia: < 10 ms. |
| 3 | TUI | Pantallas Play y Synth con medidores y edición de parámetros; entrada de teclado y joystick. |
| 4 | Patches desde archivo | Cargar y guardar sintes en TOML; recarga en caliente. |
| 5 | Secuenciador y grabación | Pantalla Seq tipo tracker; grabación de lo tocado y exportación/importación `.mid`. |
| 6 | Imagen de arranque directo | La Pi enciende y carga DistroDAW sin intervención. Decisión Buildroot vs. Pi OS Lite. |

### Fases posteriores (sin fecha)

- Pantalla Song y arreglo.
- Efectos (reverb, delay) y más voces; migración a Pi 3/4 si hace falta.
- Editor de grafo modular de sintes.
- Piano roll.
- GUI de escritorio ligera como crate alternativo.
- Soporte de PC como plataforma de primera clase (empaquetado x86_64).

## 8. Referencias de inspiración

- **Trackers:** Schism Tracker (pantallas con teclas F, muy cercano a la propuesta), LGPT (Pi, pocos botones, útil con joystick), Furnace (editor de instrumentos), GoatTracker, Klystrack, Renoise, LSDj, Polyend Tracker.
- **Híbridos secuenciador + sintes:** SunVox.
- **Terminal puro:** Orca-c (cuadrícula de caracteres con ncurses), Seq66 (`seq66cli`), reproductores de módulos en ncurses.
- **Hardware embebido:** Norns, Organelle, OP-1, Zynthian.
- **Diseño de sintes:** Pure Data, VCV Rack (grafos); SuperCollider, Csound, Sonic Pi (texto).
- **TUIs modernas bien hechas:** btop, lazygit, helix.

Hasta donde sabemos no existe un proyecto en Rust que combine DAW solo MIDI,
diseño de sintes y TUI con arranque directo, pero el estado de mantenimiento
de los proyectos anteriores no se ha verificado y conviene comprobarlo antes
de depender de ellos como referencia.

## 9. Riesgos

| Riesgo | Mitigación |
|--------|------------|
| La Pi 2 no da la polifonía deseada | Medir en el Hito 1 antes de escribir mucho código; desarrollar en PC y probar en la Pi; migrar a Pi 3/4 si hace falta. |
| Clics y cortes de audio | Disciplina de tiempo real desde el día uno: sin reservas de memoria ni locks en el hilo de audio; colas sin bloqueo. |
| Latencia alta por el jack de 3.5 mm | Usar el Volt 2 (USB) o un DAC I2S. |
| Ancho de banda USB limitado en la Pi 2 | Hub con alimentación propia; medir con todos los dispositivos conectados. |
| Grafo de sintes difícil de acertar | Empezar con voz fija y diferir el grafo. |
| Note-off ausente en el teclado de terminal | Usar `evdev` cuando se quiera el teclado como piano. |

## 10. Flujo de trabajo y gobernanza

- Licencia: **MIT**.
- Cada feature nueva va en su **propia branch**, con un commit por cada característica implementada.
- Los commits se escriben en inglés.
- Ningún commit, PR ni descripción de cambios lleva atribución a Claude (ver `CLAUDE.md`).
- Repositorio: <https://github.com/jaimevillegas/distrodaw>.

## 11. Preguntas abiertas

1. **Formato exacto de patches:** ¿TOML puro o un pequeño lenguaje propio?
2. **Imagen de arranque:** Buildroot (arranque en segundos, más trabajo) vs. Raspberry Pi OS Lite con autologin (más fácil para empezar).
3. **Modelo del secuenciador:** pistas y patrones, cantidad de canales, resolución (PPQN) y manejo de tempo.
4. **Sincronía MIDI externa:** si se soportará reloj MIDI (entrada y salida).
5. **Salida MIDI:** si el DAW también controlará sintetizadores externos.
6. **Mapeo de controles:** cómo se configura y se guarda (archivo propio o dentro del patch).
7. **Distribución:** nombre del binario, empaquetado para PC y política de versiones.
