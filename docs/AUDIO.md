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
