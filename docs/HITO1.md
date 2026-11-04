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
./bench-pi.sh Volt                  # prueba buffers 64/128/256 con 1 a 64 voces
```

Cada prueba imprime una línea `RESUMEN` con el buffer real, la latencia teórica
de salida, la carga media y pico del callback, y los plazos perdidos. El script
guarda todo en `bench-<fecha>.txt`.

También se puede lanzar una prueba suelta:

```sh
./distrodaw --device Volt --rate 48000 --buffer 64 --voices 16 --seconds 10
```

### Qué significan las métricas

- **buffer_ms:** frames del buffer / frecuencia de muestreo. Es la latencia
  teórica de un buffer, no la de ida y vuelta (no se mide con un loopback).
- **carga_media / pico:** tiempo que tarda el callback en rellenar el buffer,
  como porcentaje del tiempo que dura ese buffer. Por encima de ~70 % de pico
  hay riesgo de cortes.
- **plazos_perdidos:** callbacks que tardaron más que su propio buffer. Es una
  cota inferior de los cortes reales: cpal recupera los *underruns* de ALSA en
  silencio y no los notifica.
- Las voces son sinusoides con `sin()` en `f32`, sin filtro ni envolventes.
  Sirven de referencia relativa; una voz real del Hito 2 costará más.

## 4. Resultados

### Jack de 3,5 mm de la Pi 2 (provisional)

Medido el 2026-11-03 en una Pi 2 (ARMv7, Raspberry Pi OS 13 «trixie», 32 bits),
con el gobernador `ondemand` (900 MHz) y el dispositivo `default:CARD=Headphones`
a 44,1 kHz con el buffer por defecto. 5 s por prueba, sinusoides con `sin()`.

| Voces | Buffer (frames) | ms | Carga media | Pico | Plazos perdidos |
|-------|-----------------|----|-------------|------|-----------------|
| 8     | 2206            | 50.0 | 8.4 %     | 11.6 % | 0 |
| 16    | 2195            | 49.8 | 15.0 %    | 22.8 % | 0 |
| 32    | 2206            | 50.0 | 27.9 %    | 44.9 % | 0 |
| 64    | 2241            | 50.8 | 46.3 %    | 88.5 % | 0 |
| 128   | 1328            | 30.1 | 90.3 %    | 296.6 % | 4 |
| 256   | 1517            | 34.4 | 179.7 %   | 527.7 % | 53 |

Lectura: una sinusoide cuesta cerca de un 1 % de CPU, y hasta 64 voces
no hay plazos perdidos. Una voz real (2 osciladores, filtro, 2 envolventes,
LFO) costará varias veces más, así que la polifonía real se medirá en el
Hito 2.

### Pendiente

- **Volt 2:** no aparecía en `lsusb` ni en `aplay -l`, así que no se ha medido.
  Las mediciones con buffer de 64 a 128 frames a 48 kHz requieren el Volt 2.
- **Jack con buffer fijo:** con `--buffer` (64, 128, 256 fallan con
  `Invalid argument`; 512 y 1024 abren pero inundan de `POLLERR`) no funciona
  en `plughw:CARD=Headphones`. Solo va el buffer por defecto (~50 ms).
- **CPU a frecuencia fija:** el gobernador `performance` necesita `sudo`; las
  cifras anteriores se midieron con `ondemand`.
