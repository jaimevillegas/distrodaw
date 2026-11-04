# Kiosco con color de 24 bits (cage + foot) y temas

La consola del kernel (`tty1`) solo tiene **16 colores** (su paleta se puede cambiar, pero no
admite 24 bits). Para temas como Dracula o Solarized con sus colores exactos hace falta un
terminal gráfico. La solución elegida es:

- **cage**: un compositor Wayland mínimo que muestra un solo programa a pantalla completa
  (sin escritorio, sin ventanas).
- **foot**: un terminal ligero con color de 24 bits y fuentes TTF a cualquier tamaño.

Se descartaron `kmscon` (más ligero pero con menos soporte y pruebas) y `fbterm` (solo 256 colores).

> **Estado:** los scripts y los temas están probados en el PC (con un `cage` simulado) y la
> interfaz emite colores de 24 bits correctos, pero **cage y foot aún no se han instalado ni
> probado en la Pi 2**. Por eso el arranque automático es opcional y se prueba antes a mano.

## Antes de empezar

1. **Modo de pantalla.** Si el monitor se enciende después del arranque, el kernel puede
   quedarse con la lista de modos de respaldo (1024x768) aunque el monitor anuncie 1080p. Cage
   usa el modo del kernel. Comprueba que el modo preferido sea el bueno:

   ```sh
   cat /sys/class/drm/card0-HDMI-A-1/modes | head -3    # el primero debe ser 1920x1080
   ```

   Si no, lo normal es que baste con **reiniciar con el monitor ya encendido y conectado**
   (o volver a conectar el cable HDMI). Solo si sigue fallando, fuerza el modo con
   ` video=HDMI-A-1:1920x1080@60` al final de la línea de `/boot/firmware/cmdline.txt`.

   > **Cuidado con `cmdline.txt`:** va en la partición de arranque (FAT), que es frágil ante
   > cortes de corriente. Si queda vacío, la Pi puede no arrancar. Haz siempre copia y
   > comprueba el resultado antes de reiniciar:
   >
   > ```sh
   > sudo cp /boot/firmware/cmdline.txt /boot/firmware/cmdline.txt.bak
   > # ...editar...
   > sync && wc -c /boot/firmware/cmdline.txt     # debe tener unos 170 bytes, no 0
   > ```
   >
   > Apaga siempre con `sudo shutdown -h now` o `sudo reboot`; no quites la alimentación sin más.
2. **Alimentación.** Un hub USB con alimentación propia para el Volt 2 y los teclados
   (ver `docs/AUDIO.md`). El compositor añade algo de carga a la Pi.

## Pasos

En el PC, desde el repositorio:

```sh
scripts/deploy-pi.sh distrodaw@192.168.1.64 distrodaw
```

En la Pi (por SSH), en la carpeta `~/distrodaw`:

```sh
./pi-kiosk.sh install       # instala cage, foot y las fuentes (pide sudo) y crea kiosk.conf
./pi-kiosk.sh status
```

Edita `kiosk.conf` (tema, fuente, tamaño, dispositivo de audio, puerto MIDI, patch).

**Prueba a mano**, en la consola de la Pi (monitor y teclado, no por SSH):

```sh
cd ~/distrodaw
./pi-kiosk.sh try
```

Debe abrir DistroDAW a pantalla completa con el fondo del tema. Pulsa `q` para salir. Dentro
de un terminal de 24 bits, `./pi-kiosk.sh colortest` debe mostrar degradados suaves.

**Comprueba que el audio no se resiente.** Con el kiosco abierto, la TUI muestra `cpu` y
`xruns` en el pie. Toca con ocho notas sostenidas unos minutos: `xruns` debe seguir en 0 y
`cpu` debe quedar en pocos puntos porcentuales por encima de lo que daba la consola.

**Arranque automático** cuando todo esté bien:

```sh
./pi-kiosk.sh enable        # autologin en tty1 + un bloque en ~/.profile (pide confirmación)
sudo reboot
```

`enable` solo afecta a la pantalla física (`tty1`): SSH y las demás consolas siguen igual. Al
pulsar `q` en DistroDAW, la sesión se reinicia sola y vuelve a abrirlo.

### Volver atrás o entrar a la consola normal

```sh
./pi-kiosk.sh disable                 # deshace el autologin y el bloque de ~/.profile
touch ~/distrodaw/NOKIOSK             # por SSH: el kiosco no arranca (borra el archivo para reactivarlo)
```

Con el kiosco abierto, `Ctrl+Alt+F2` cambia a otra consola con el login normal.

## Configuración (`kiosk.conf`)

| Variable | Qué hace |
|----------|----------|
| `THEME` | Archivo de `themes/` (sin `.toml`) |
| `FONT`, `FONT_SIZE` | Fuente de foot y su tamaño (más grande = letras más grandes) |
| `AUDIO_ARGS` | Opciones de audio antes del subcomando (`--device`, `--rate`, `--buffer`) |
| `TUI_ARGS` | Opciones de la interfaz (`--midi`, `--patch`...) |
| `WLR_RENDERER=pixman` | Descomentar si cage falla con el renderizador de GPU |

El fondo y el color de texto de foot se toman del tema, para que no se note el borde de la ventana.

## Temas

`distrodaw tui --theme dracula` carga `themes/dracula.toml` (o una ruta a un `.toml`).
Incluye `dracula`, `solarized-dark`, `gruvbox-dark` y `nord`. Cada tema da un color a cada
papel de la interfaz:

```toml
name = "Dracula"

[colors]
background = "#282a36"
foreground = "#f8f8f2"
title      = "#bd93f9"   # títulos de pantalla
label      = "#6272a4"   # etiquetas, bordes y textos secundarios
value      = "#50fa7b"   # valores y barras
accent     = "#f1fa8c"   # resaltes (cadena de señal)
ok         = "#50fa7b"   # medidor y estado correcto
warn       = "#ffb86c"   # medidor alto (a partir del 70 %)
alert      = "#ff5555"   # errores y medidor al máximo (a partir del 90 %)
```

Los colores que falten toman los de Dracula; una clave desconocida o un color mal escrito es un
error con el nombre del campo. Para crear un tema, copia uno y cambia los colores.

### Cuántos colores usa la interfaz

`--color auto` (por defecto) usa 24 bits si el terminal lo anuncia con `COLORTERM=truecolor`
(foot lo hace solo) y 16 colores en caso contrario. `--color truecolor` lo fuerza (útil por SSH
desde un terminal de escritorio moderno) y `--color basic` fuerza los 16 colores. En la consola
del kernel (16 colores) se ignora el tema y se usan los colores básicos, y la interfaz avisa.

## Si algo falla

- **Pantalla negra al hacer `try`:** el error sale en la consola al volver. Prueba con
  `export WLR_RENDERER=pixman` en `kiosk.conf`. Cage necesita una sesión de usuario normal en
  `tty1` (con `loginctl` debe verse una sesión en `seat0`).
- **Letras demasiado grandes o pequeñas:** cambia `FONT_SIZE`.
- **Resolución equivocada:** ver «Antes de empezar», punto 1.
- **No quieres el kiosco al arrancar:** `./pi-kiosk.sh disable` o el archivo `NOKIOSK`.
