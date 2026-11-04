# Hito 4 — Patches desde archivo

Objetivo (ver `SPEC.md`): cargar y guardar sintes en TOML, con recarga en caliente.
Se resuelve la pregunta abierta 1: **TOML puro**, sin lenguaje propio.

## El formato

```toml
name = "Pad"

[oscillator]
waveform = "triangle"  # sine, saw, square o triangle

[envelope]
attack = 0.6    # segundos
decay = 0.8
sustain = 0.8   # 0.0 a 1.0
release = 1.5

[output]
volume = 0.5    # 0.0 a 1.0
```

- Todo es opcional: lo que falte toma el valor por defecto (un archivo vacío es el patch `Init`).
- Una **clave desconocida** o un **valor fuera de rango** es un error (`envelope.sustain = 9
  fuera de rango (0 a 1)`), para que una errata al editar a mano se vea en vez de ignorarse.
  Consecuencia: un patch con secciones que esta versión no conoce (p. ej. un futuro filtro) no cargará.
- Las secciones se corresponden con módulos del sinte, de modo que el filtro, el LFO o un
  segundo oscilador llegarán como secciones nuevas sin romper los patches existentes.
- Rangos: `attack`, `decay` y `release` de 0,001 a 5 s; `sustain` y `volume` de 0 a 1.

En `patches/` hay seis ejemplos (`init`, `pad`, `pluck`, `lead`, `bass`, `organ`); un test
comprueba que todos son válidos.

## Uso

```sh
distrodaw tui --patch patches/pad.toml            # carga al arrancar y recarga al guardar
distrodaw tui --patch-dir ~/mis-patches           # carpeta de la pantalla Files (por defecto `patches`)
distrodaw play --patch patches/lead.toml          # modo de texto: solo carga al arrancar
```

### Recarga en caliente

Con `--patch`, o tras cargar un archivo desde Files, se vigila ese archivo. Edítalo con
cualquier editor (también por SSH en la Pi) y al guardar el sonido cambia sin parar el audio.

- Se mira el disco cuatro veces por segundo y un cambio solo se aplica cuando el archivo se
  mantiene igual en dos comprobaciones seguidas, para no leer algo que el editor aún escribe.
- Si el archivo es inválido se muestra el error en rojo (en la parte superior de cualquier
  pantalla) y **se mantiene el sonido anterior**. Al corregirlo se recarga solo.
- Si el patch de `--patch` no carga al arrancar, la interfaz abre igualmente y sigue
  vigilando el archivo.
- Las notas que ya suenan siguen sonando con los parámetros nuevos.

### Pantalla Files (F5)

| Tecla | Acción |
|-------|--------|
| ↑ ↓ | Elegir patch |
| Enter | Cargar el seleccionado (y empezar a vigilarlo) |
| `s` | Guardar los parámetros actuales sobre el seleccionado |
| `n` | Guardar como archivo nuevo (`patch-001.toml`, `patch-002.toml`...) |

El ● marca el patch cargado. Los valores guardados llevan hasta cuatro decimales. Si el patch
actual no tiene nombre propio (`Init`), se guarda con el nombre del archivo. La lista se
refresca cada segundo, así que los archivos creados desde fuera aparecen solos.

## Cómo está hecho

- **`engine::patch`** convierte texto ↔ `Patch` (nombre + parámetros). No toca el disco.
- **`app::patches`** lista, lee, guarda (escribe un temporal y lo renombra, para que nadie
  lea un archivo a medias) y vigila archivos; y envía los seis parámetros al motor como
  eventos por la misma cola que usa la interfaz.
- La interfaz no toca el disco: pide una `FileRequest` y la app la atiende.
- `scripts/deploy-pi.sh` también copia `patches/` a la Pi.

## Pruebas

- [x] Tests: 9 de formato (lectura, errores, ida y vuelta, redondeo), 7 de archivos y
      vigilancia (con carpetas temporales), 9 nuevos de la pantalla Files y los mensajes.
- [x] PC y Pi 2 en un terminal virtual: arrancar con `--patch`, editar el archivo desde
      fuera (release 1,5 s → 300 ms se aplica), meter un valor inválido (error visible y el
      sonido se mantiene) y corregirlo (se recarga).
- [x] Pantalla Files: listar, cargar un patch, editar un parámetro y guardar uno nuevo;
      el archivo resultante se vuelve a leer sin errores.
- [ ] Probar a oído con el teclado: cambiar un patch por SSH mientras se toca.
