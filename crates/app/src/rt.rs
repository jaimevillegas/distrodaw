//! Prioridad de tiempo real para los hilos de audio y MIDI.
//!
//! Con prioridad normal, el hilo de audio compite en igualdad con la interfaz,
//! el terminal gráfico y el compositor, y puede tardar milisegundos en volver a
//! ejecutarse: con un buffer de 2,7 ms eso se oye como un corte. `SCHED_FIFO`
//! hace que el sistema lo ejecute antes que cualquier hilo normal.
//!
//! Hace falta permiso: en Debian se concede al grupo `audio` con
//! `/etc/security/limits.d` (lo hace `pi-kiosk.sh tune`). Sin él, la llamada falla
//! con `EPERM` y el programa sigue funcionando con prioridad normal.

use std::io;

/// Prioridad del hilo de audio (1 a 99).
pub const AUDIO_PRIORITY: i32 = 70;
/// Prioridad del hilo MIDI: justo por debajo de la del audio, pero por encima
/// de lo normal, para que las notas no esperen a la interfaz. `audio` es la
/// prioridad del hilo de audio; con 0 (desactivado) no se pide nada.
pub fn midi_priority(audio: i32) -> i32 {
    if audio <= 0 {
        0
    } else {
        (audio - 10).clamp(1, 99)
    }
}

/// Pide al sistema que el hilo que llama pase a `SCHED_FIFO` con `priority`.
#[cfg(target_os = "linux")]
pub fn promote(priority: i32) -> io::Result<()> {
    let param = libc::sched_param { sched_priority: priority };
    // SAFETY: `param` es una estructura válida durante toda la llamada y el
    // pid 0 designa al hilo que llama.
    let rc = unsafe { libc::sched_setscheduler(0, libc::SCHED_FIFO | libc::SCHED_RESET_ON_FORK, &param) };
    if rc == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

#[cfg(not(target_os = "linux"))]
pub fn promote(_priority: i32) -> io::Result<()> {
    Err(io::Error::new(io::ErrorKind::Unsupported, "tiempo real no soportado en este sistema"))
}

#[cfg(test)]
mod priority_tests {
    use super::*;

    #[test]
    fn midi_runs_just_below_audio_and_never_when_disabled() {
        assert_eq!(midi_priority(70), 60);
        assert_eq!(midi_priority(5), 1);
        assert_eq!(midi_priority(0), 0);
        assert_eq!(midi_priority(-3), 0);
    }
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    fn policy() -> i32 {
        // SAFETY: consulta de solo lectura sobre el hilo que llama.
        unsafe { libc::sched_getscheduler(0) }
    }

    #[test]
    fn promotion_either_works_or_fails_cleanly_without_changing_the_thread() {
        // En un hilo propio para no alterar el del test.
        let result = std::thread::spawn(|| {
            let before = policy();
            let outcome = promote(10);
            (before, outcome.map_err(|e| e.raw_os_error()), policy())
        })
        .join()
        .unwrap();
        match result {
            (_, Ok(()), after) => assert_eq!(after & !libc::SCHED_RESET_ON_FORK, libc::SCHED_FIFO),
            (before, Err(errno), after) => {
                // Sin permiso (EPERM) o sin soporte: el hilo queda como estaba.
                assert!(matches!(errno, Some(libc::EPERM) | Some(libc::EINVAL) | Some(libc::EACCES)), "{errno:?}");
                assert_eq!(before, after);
            }
        }
    }

    #[test]
    fn out_of_range_priority_is_rejected() {
        let result = std::thread::spawn(|| promote(1000)).join().unwrap();
        assert!(result.is_err());
    }
}
