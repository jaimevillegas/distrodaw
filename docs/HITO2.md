# Hito 2 — MIDI y sinte polifónico

Objetivo (ver `SPEC.md`): teclado MIDI → sinte de 8 voces con ADSR, controlado
por parámetros con ID, con eventos con marca de tiempo y latencia < 10 ms.

## Qué hay

**`crates/engine`** (sin I/O, apto para el hilo de audio):

| Módulo | Contenido |
|--------|-----------|
| `params` | `ParamId`, rango y valor por defecto de cada parámetro: `waveform`, `attack`, `decay`, `sustain`, `release`, `volume`. |
| `event` | `Event { frame, kind }` con `NoteOn`, `NoteOff` y `SetParam`. `frame` es la posición dentro del buffer. |
| `env` | Envolvente ADSR con coeficientes precalculados (el bucle de audio solo suma y multiplica). |
| `voice` | Oscilador (seno, sierra, cuadrada, triangular) + ADSR. Las formas de onda son ingenuas, sin anti-aliasing. |
| `synth` | 8 voces. Reutiliza la voz de la misma nota, si no una libre, si no roba la más antigua (prefiere las que ya están soltando). |

**`crates/app`**:

- `distrodaw play`: teclado MIDI → cola `rtrb` sin bloqueo → sinte → audio.
- `distrodaw midi-ports`: lista los puertos de entrada MIDI.
- `distrodaw bench --synth`: mide el coste del sinte real con 8 notas sostenidas.

## Tocar

```sh
distrodaw midi-ports
distrodaw play --device hw:CARD=V2,DEV=0 --rate 48000 --buffer 128
distrodaw play --midi <fragmento-del-puerto> --monitor    # muestra cada mensaje
```

Sin `--midi` se usa el primer puerto que no sea «Midi Through». Se ignora el
canal MIDI (modo omni).

### Controladores MIDI asignados

| CC | Parámetro |
|----|-----------|
| 7  | volume |
| 70 | waveform (0 seno, 1 sierra, 2 cuadrada, 3 triangular) |
| 72 | release |
| 73 | attack |
| 75 | decay |
| 79 | sustain |

Los tiempos (attack, decay, release) usan una curva cuadrática para tener más
resolución en valores cortos.

## Marcas de tiempo

El motor aplica cada evento en su `frame` dentro del buffer (probado en los tests
del motor). Los eventos que llegan por MIDI en vivo se estampan en el frame 0
del siguiente trozo de audio, porque su instante exacto de llegada aún no se
traslada a frames. Con buffers de 64 a 128 frames el error es de menos de un
trozo (≈ 0,4 a 1,3 ms). Cuando exista el secuenciador (Hito 5) usará `frame`
para colocar las notas con precisión.

## Resultados en la Pi 2

Pi 2 con el Volt 2 (`hw:CARD=V2,DEV=0`), 48 kHz, gobernador `performance`,
4 s por prueba. Sinte con 8 notas sostenidas (acorde de 8 notas). Plazos
perdidos y errores: 0.

| Buffer (frames) | ms | Forma de onda | Carga media | Pico (ms) |
|-----------------|----|---------------|-------------|-----------|
| 64  | 1.33 | seno   | 8.3 % | 0.16 |
| 64  | 1.33 | sierra | 2.9 % | 0.05 |
| 128 | 2.67 | seno   | 8.0 % | 0.28 |
| 128 | 2.67 | sierra | 2.7 % | 0.08 |
| 256 | 5.33 | seno   | 7.9 % | 0.46 |
| 256 | 5.33 | sierra | 2.7 % | 0.13 |

Las 8 voces cuestan entre un 3 % (sierra) y un 8 % (seno, por el `sin()`) de la
Pi 2, así que hay margen para más voces o para un filtro.

### Latencia

Estimación, **no medida**: latencia del teclado USB (~1 ms) + espera hasta el
siguiente trozo de audio (< 1,3 ms) + buffer de salida (1,33 a 2,67 ms con 64 a
128 frames) + transporte USB del Volt 2 (~1 ms) ≈ 4 a 6 ms, dentro de la meta
de 10 ms. Medirla de verdad requiere un loopback (salida del Volt 2 a su
entrada) y un pulso MIDI, y queda pendiente.

## Pruebas hechas y pendientes

- [x] Tests del motor (32) y de la app (9): parámetros, eventos, ADSR, voces,
      robo de voces, marcas de tiempo, interpretación MIDI y mapa de CC.
- [x] `play` extremo a extremo en el PC y en la Pi 2 enviando notas y CC por el
      puerto virtual «Midi Through» (`aseqsend`): llegan al motor y suenan las voces.
- [ ] Probar con el teclado MIDI USB real conectado a la Pi.
- [ ] Medir la latencia real (loopback).
