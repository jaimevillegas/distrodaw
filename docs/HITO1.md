# Hito 1 — Compilar a la Pi 2 y medir

Objetivo (ver `SPEC.md`): sinusoide por el Volt 2 en la Pi 2, con latencia de
buffer de 64 a 128 frames a 48 kHz y uso de CPU registrado. El resultado define
cuántas voces caben.

## 1. Preparar la Pi 2

1. Con Raspberry Pi Imager, instala **Raspberry Pi OS Lite (32-bit)**. En los
   ajustes avanzados define hostname, usuario, red y activa SSH con tu clave pública.
2. Por SSH:

   ```sh
   sudo apt update && sudo apt full-upgrade -y
   sudo apt install -y alsa-utils
   sudo usermod -aG audio,input "$USER"
   ```

   Cierra la sesión y vuelve a entrar para que se apliquen los grupos.
3. Conecta el Volt 2 (mejor a un hub con alimentación propia) y comprueba que
   aparece con `aplay -l`.
4. Para medir con la CPU a frecuencia fija:

   ```sh
   echo performance | sudo tee /sys/devices/system/cpu/cpu0/cpufreq/scaling_governor
   ```

## 2. Compilar y copiar

Desde el PC (necesita `cross` y Docker):

```sh
scripts/deploy-pi.sh usuario@host
```

Compila para `armv7-unknown-linux-gnueabihf` y copia `distrodaw` y
`bench-pi.sh` a `~/distrodaw` en la Pi.

## 3. Medir

En la Pi:

```sh
cd ~/distrodaw
./distrodaw --list-devices          # busca el nombre del Volt 2
./bench-pi.sh hw:CARD=V2,DEV=0      # prueba buffers 64/128/256 con 1 a 64 voces
```

Cada prueba imprime una línea `RESUMEN` con el buffer real, la latencia teórica
de salida, la carga media y pico del callback, y los plazos perdidos. El script
guarda todo en `bench-<fecha>.txt`.

También se puede lanzar una prueba suelta:

```sh
./distrodaw --device hw:CARD=V2,DEV=0 --rate 48000 --buffer 64 --voices 16 --seconds 10
```

### Qué significan las métricas

- **buffer_ms:** buffer pedido con `--buffer` / frecuencia de muestreo. Es la
  latencia teórica de salida del buffer, no la de ida y vuelta (no se mide con
  un loopback) ni incluye el transporte USB. `callback_frames` es otra cosa:
  cpal entrega el audio en trozos más pequeños que el buffer (18 a 126 frames).
- **pico_ms:** el callback más lento de la prueba, en milisegundos absolutos.
  Si supera a `buffer_ms` hubo corte seguro; es la cifra más fiable.
- **carga_media / pico:** tiempo que tarda el callback en rellenar su trozo,
  como porcentaje del tiempo que dura ese trozo. La carga media es fiable; el
  pico en % se mide contra el trozo, no contra el buffer, y puede pasar de
  100 % sin cortes (mira `pico_ms`).
- **plazos_perdidos:** callbacks que tardaron más que el trozo que rellenaban. Es una
  cota inferior de los cortes reales: cpal recupera los *underruns* de ALSA en
  silencio y no los notifica.
- Las voces son sinusoides con `sin()` en `f32`, sin filtro ni envolventes.
  Sirven de referencia relativa; una voz real del Hito 2 costará más.

## 4. Resultados

### Volt 2 en Pi 2 (Hito 1)

Medido el 2026-11-03 en una Pi 2 (ARMv7, Raspberry Pi OS 13 «trixie», 32 bits)
con el Volt 2 por `hw:CARD=V2,DEV=0` (I16, 2 canales) a 48 kHz, con el
gobernador `ondemand` (900 MHz). 5 s por prueba, sinusoides con `sin()`.
Plazos perdidos y errores: 0 en todas las pruebas.

| Buffer (frames) | ms | Voces | Carga media | Pico (ms) |
|-----------------|----|-------|-------------|-----------|
| 64  | 1.33 | 1  | 1.4 %  | 0.12 |
| 64  | 1.33 | 8  | 8.2 %  | 0.15 |
| 64  | 1.33 | 16 | 15.3 % | 0.21 |
| 64  | 1.33 | 32 | 25.7 % | 0.37 |
| 64  | 1.33 | 64 | 51.2 % | 0.69 |
| 128 | 2.67 | 1  | 1.6 %  | 0.14 |
| 128 | 2.67 | 8  | 8.3 %  | 0.27 |
| 128 | 2.67 | 16 | 16.0 % | 0.55 |
| 128 | 2.67 | 32 | 31.2 % | 0.70 |
| 128 | 2.67 | 64 | 50.0 % | 1.33 |
| 256 | 5.33 | 1  | 1.5 %  | 0.11 |
| 256 | 5.33 | 8  | 8.7 %  | 0.39 |
| 256 | 5.33 | 16 | 16.2 % | 0.70 |
| 256 | 5.33 | 32 | 31.6 % | 1.38 |
| 256 | 5.33 | 64 | 50.8 % | 2.74 |

Lectura:

- Una sinusoide cuesta cerca de un 0,8 % de CPU. Con 64 voces la carga media es
  del 50 % y el peor callback (0,69 a 2,74 ms) cabe en el buffer en todos los casos.
- Los buffers de 64 y 128 frames funcionan sin cortes, así que el criterio de
  latencia del hito (64 a 128 frames a 48 kHz, 1,3 a 2,7 ms de buffer) se cumple.
- Una voz real (2 osciladores, filtro, 2 envolventes, LFO) costará varias veces
  más que una sinusoide. La polifonía real se medirá en el Hito 2.

### Jack de 3,5 mm (referencia)

El jack solo funciona con el buffer por defecto (~50 ms): con `--buffer`, los
tamaños 64, 128 y 256 fallan con `Invalid argument`, y 512 y 1024 abren pero
inundan de `POLLERR`. Con 64 sinusoides la carga media es del 46 %, y con 128
ya hay plazos perdidos.

### Avisos

- **Subtensión:** `vcgencmd get_throttled` da `0x50000` (hubo subtensión y
  limitación de CPU desde el arranque). El kernel la registró al conectar el
  Volt 2. Conviene una fuente de 5 V / 2,5 A o un hub con alimentación propia
  antes de dar estas cifras por definitivas.
- **CPU a frecuencia fija:** el gobernador `performance` necesita `sudo`; las
  cifras se midieron con `ondemand`.
- **No medido:** latencia de ida y vuelta (requiere loopback) y comportamiento
  con teclado MIDI, joystick y demás dispositivos USB conectados a la vez.
