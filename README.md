# DistroDAW

DAW **solo MIDI** para terminal, escrito en Rust y pensado para arrancar directo
sobre un kernel de Linux en una Raspberry Pi (objetivo inicial: Pi 2), pero
compilable también para PC x86_64. Software libre, licencia [MIT](LICENSE):
cópialo, modifícalo y úsalo para lo que quieras.

## Estado

Hito 1 completado: banco de pruebas de audio (`distrodaw --help`) y mediciones en la
Pi 2 con el Volt 2 (buffers de 64 a 256 frames, hasta 64 sinusoides sin cortes).
Ver [docs/HITO1.md](docs/HITO1.md).

Hito 2 implementado: sinte polifónico de 8 voces con ADSR y parámetros con ID,
controlado por teclado MIDI (`distrodaw play`). Probado con un puerto MIDI virtual y
con un teclado USB real en la Pi 2; pendiente medir la latencia real. Ver [docs/HITO2.md](docs/HITO2.md).

Hito 3 completado: interfaz de terminal con pantallas Play y Synth (`distrodaw`), manejada
con teclado (el joystick queda como opcional para el futuro). Ver [docs/HITO3.md](docs/HITO3.md).

Hito 4 implementado: sintes como patches en TOML (`patches/`), con recarga en caliente y
pantalla Files (F5) para cargar y guardar. Pendiente de probar a oído. Ver
[docs/HITO4.md](docs/HITO4.md).

Color de 24 bits y temas (`--theme dracula`, `solarized-dark`, `gruvbox-dark`, `nord`) y un kiosco
cage + foot para la Pi, probado en la Pi 2. Ver [docs/KIOSK.md](docs/KIOSK.md). El audio se
reabre solo si el dispositivo se desconecta: [docs/AUDIO.md](docs/AUDIO.md).

## Estructura

| Crate | Función |
|-------|---------|
| `crates/engine` | Sintes, voces y secuenciador. Sin I/O: portable y testeable. |
| `crates/tui`    | Interfaz de terminal (ratatui + crossterm). Pendiente. |
| `crates/app`    | Binario `distrodaw`: audio (cpal), MIDI (midir) y eventos. |

## Compilar y probar en PC

Requisitos: Rust estable y, en Linux, `libasound2-dev` (Debian/Ubuntu) o
`alsa-lib-devel` (Fedora).

```sh
cargo test -p distrodaw-engine
cargo run --release -p distrodaw -- devices
cargo run --release -p distrodaw -- midi-ports
cargo run --release -p distrodaw -- play --monitor
cargo run --release -p distrodaw            # interfaz de terminal
cargo run --release -p distrodaw -- tui --patch patches/pad.toml   # con un patch que se recarga al guardar
cargo run --release -p distrodaw -- tui --theme dracula --color truecolor   # color de 24 bits
cargo run --release -p distrodaw -- bench --buffer 128 --rate 48000 --voices 16 --seconds 5
```

## Compilar para Raspberry Pi 2 (ARMv7)

```sh
cargo install cross
cross build --release --target armv7-unknown-linux-gnueabihf -p distrodaw
scripts/deploy-pi.sh pi@raspberrypi.local   # compila y copia binario y banco de pruebas
```

`cross` necesita Docker o Podman. La configuración de `libasound2` para ARM
está en `Cross.toml`.

## Principios de diseño

1. **Parámetros con identidad:** cada control tiene ID, nombre, rango y valor;
   TUI, joystick, MIDI CC y patches controlan los mismos parámetros.
2. **Eventos con marca de tiempo** en frames dentro del buffer de audio.
3. **Voz fija primero** (2 osciladores, filtro, 2 envolventes, LFO); el grafo
   modular llega después.
4. **Hilo de audio sin reservas de memoria ni locks.**

## Hitos

- [x] 0. Oscilador sonando en PC
- [x] 1. Compilar a Pi 2 y medir latencia/CPU con el Volt 2
- [x] 2. Teclado MIDI → sinte de 8 voces con ADSR
- [x] 3. TUI (Play, Synth; joystick opcional más adelante)
- [ ] 4. Patches desde archivo (TOML)
- [ ] 5. Secuenciador tipo tracker y grabación a .mid
- [ ] 6. Imagen de arranque directo (Buildroot o Raspberry Pi OS Lite)

## Licencia

MIT. Ver [LICENSE](LICENSE).
