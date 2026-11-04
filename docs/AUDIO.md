# Audio: errores, xruns y pérdida del dispositivo

## Qué pasa si el dispositivo de audio desaparece

Con cpal 0.15 el flujo moría en silencio: el hilo de audio quedaba girando al 100 % de CPU,
las notas MIDI seguían llegando a la interfaz pero el motor no procesaba nada (voces siempre
a 0) y los cambios de parámetro con ←/→ no se veían, porque la pantalla volvía a leer los
valores congelados del motor.

Ahora (cpal 0.18):

1. El controlador avisa con `DeviceNotAvailable` y **el hilo de audio termina solo**.
2. La sesión detecta el aviso, suelta el flujo y **recupera el motor** (sinte, parámetros y
   colas) por un canal, porque el callback se lo devuelve al destruirse.
3. Reintenta abrir el dispositivo cada segundo. Al volver lo reabre con el **mismo motor**:
   los parámetros se conservan y se envía «todas las notas fuera» para no dejar notas
   colgadas.

En la interfaz se ve en rojo `AUDIO PERDIDO · se reintenta solo` en el pie, y al volver
«Audio recuperado». En `distrodaw play` se imprime una línea equivalente.

Si la frecuencia de muestreo del dispositivo cambia al volver, el sinte se recrea con los
mismos parámetros.

El camino de recuperación se prueba con un dispositivo real (simulando la pérdida):

```sh
cargo test -p distrodaw -- --ignored
```

## Xruns

cpal 0.18 notifica los subdesbordamientos (`Xrun`). El contador `xruns` del pie suma esos
avisos, los plazos perdidos (callbacks que tardan más que su trozo) y otros errores.

## Subtensión en la Pi

Si `dmesg` muestra `Undervoltage detected!` seguido de `USB disconnect` del Volt 2, la fuente
no aguanta: el Volt 2 (alimentado por USB), el teclado MIDI, un teclado normal y el adaptador
Wi-Fi comparten el bus y la corriente de la Pi 2. Es la causa de fondo de las caídas de audio.
Lo recomendable es una fuente de 5 V / 2,5 A de calidad con un cable corto y un **hub USB con
alimentación propia** para el Volt 2 y los teclados.

```sh
vcgencmd get_throttled   # 0x0 = bien; los bits 0 y 2 indican subtensión o limitación ahora
dmesg | grep -i -E "undervolt|usb disconnect"
```

## Dispositivos

`distrodaw devices` muestra el identificador (`hw:CARD=V2,DEV=0`) y entre corchetes el
nombre legible. `--device` acepta un fragmento de cualquiera de los dos, así que
`--device Volt` equivale a `--device hw:CARD=V2,DEV=0`.

## Cortes de sonido al tocar

Si se oyen cortes pero `xruns` sigue en 0, no es un subdesbordamiento que ALSA haya notado:
puede ser el hilo de audio que no se ejecutó a tiempo, o un fallo del bus USB o de la
alimentación, que ALSA no ve. El pie de la TUI y el resumen de `bench` separan los casos:

| Contador | Qué mide | Si sube |
|----------|----------|---------|
| `xruns` | Subdesbordamientos que notifica el controlador de audio, más plazos perdidos y errores | El buffer se quedó sin datos: CPU o planificación |
| `tarde` | Callbacks que llegaron con más de 2 periodos de retraso | El hilo de audio tardó en ejecutarse (competencia con otros procesos) |
| `hueco` | Mayor intervalo entre dos callbacks seguidos | Con un buffer de 2,7 ms, un hueco de 10 ms o más se oye |
| `T. real` (pantalla Play) | Si el hilo de audio tiene prioridad `SCHED_FIFO` | `NO: falta permiso` → ejecutar `pi-kiosk.sh tune` |

Si hay cortes con `xruns`, `tarde` y `hueco` en cero, el problema está fuera del programa:
alimentación (subtensión), el bus USB o el propio dispositivo.

### Causas encontradas en la Pi 2

1. **Subtensión continua.** `vcgencmd get_throttled` daba `0x50005`: el bit 0 es subtensión
   *ahora* y el bit 2 es limitación *ahora* (los `0x50000` son solo historial). La CPU pasaba la
   mayor parte del tiempo a 700 MHz en vez de 900. Es la causa de fondo: con la tensión por
   debajo del umbral también se vuelve inestable el USB. Hace falta una fuente de 5 V / 2,5 A de
   calidad y un cable corto y grueso; los hubs con alimentación propia no la sustituyen.
2. **Sin prioridad de tiempo real.** El hilo de audio corría con prioridad normal, igual que la
   interfaz, `foot` y `cage`. Con carga competidora, un callback que trabaja 0,3 ms llegó a
   tardar 8–12 ms en terminar (medido con `bench --stress`).
3. **El gobernador volvía a `ondemand` en cada reinicio**, y la frecuencia sube con retraso al
   empezar a tocar.
4. **La interfaz gastaba CPU sin necesidad:** ~28 % de un núcleo con la pantalla quieta en una
   consola de 200x50, porque ratatui compara toda la pantalla en cada cuadro y una Pi 2 es lenta.

### Qué se cambió

- El hilo de audio pide `SCHED_FIFO` (prioridad 70) en su primer callback y el de MIDI 60. Se
  ajusta con `--rt-priority` (0 lo desactiva). Funciona con `hw:` y con `plughw:`.
- La interfaz solo se redibuja cuando algo visible cambia, a 25 cuadros por segundo como máximo,
  y el medidor se actualiza a 12,5 Hz. Al terminal quieto cuesta ~0,5 % de un núcleo (antes
  9,5 % a 80x25 y 28 % a 200x50) y con notas sonando ~4 % y ~12 % (antes ~10 % y ~28 %).
- `./pi-kiosk.sh tune` concede el permiso de tiempo real al grupo `audio`
  (`/etc/security/limits.d`) y fija el gobernador en `performance` al arrancar. Cierra la sesión
  y vuelve a entrar (o reinicia) después.

### Cómo comprobar

```sh
./pi-kiosk.sh status                    # ¿tiempo real configurado? ¿gobernador?
vcgencmd get_throttled                  # 0x0 = sin subtensión
./distrodaw bench --device plughw:CARD=V2,DEV=0 --rate 48000 --buffer 128 --synth --stress 4 --seconds 10
```

`--stress N` lanza N hilos que consumen CPU a prioridad normal: con tiempo real activo,
`tarde` y `hueco` deben seguir bajos aunque la Pi esté ocupada.
