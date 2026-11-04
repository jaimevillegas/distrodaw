//! Archivos de patches: listar, leer, guardar y vigilar cambios.

use distrodaw_engine::{Event, ParamId, Patch};
use rtrb::Producer;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

const EXTENSION: &str = "toml";
/// Cada cuánto se mira si el archivo vigilado cambió.
const POLL_EVERY: Duration = Duration::from_millis(250);

/// Lee y valida un patch. El error ya incluye el nombre del archivo.
pub fn load(path: &Path) -> Result<Patch, String> {
    let name = path.file_name().map_or_else(|| path.display().to_string(), |n| n.to_string_lossy().into_owned());
    let text = fs::read_to_string(path).map_err(|e| format!("{name}: {e}"))?;
    Patch::from_toml(&text).map_err(|e| format!("{name}: {e}"))
}

/// Guarda escribiendo primero un archivo temporal y renombrándolo, para que
/// quien vigile el archivo nunca lo lea a medias.
pub fn save(path: &Path, patch: &Patch) -> io::Result<()> {
    let tmp = path.with_extension("toml.tmp");
    fs::write(&tmp, patch.to_toml())?;
    fs::rename(&tmp, path)
}

/// Patches (`*.toml`) de un directorio, ordenados por nombre.
pub fn list(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e == EXTENSION))
        .collect();
    files.sort();
    Ok(files)
}

/// Primer `patch-001.toml`, `patch-002.toml`... que no exista en `dir`.
pub fn new_file_path(dir: &Path) -> PathBuf {
    (1..)
        .map(|n| dir.join(format!("patch-{n:03}.{EXTENSION}")))
        .find(|p| !p.exists())
        .expect("siempre hay un número libre")
}

/// Nombres de archivo de los patches de `dir`; vacío si la carpeta no existe.
pub fn file_names(dir: &Path) -> Vec<String> {
    list(dir)
        .unwrap_or_default()
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().into_owned()))
        .collect()
}

/// Envía todos los parámetros del patch al motor. Si la cola está llena se
/// pierde lo que no quepa; volver a cargar el patch lo completa.
pub fn send(patch: &Patch, queue: &mut Producer<Event>) {
    for id in ParamId::ALL {
        let _ = queue.push(Event::set_param(0, id, patch.params.get(id)));
    }
}

/// Huella de un archivo: cambia si se modifica su contenido o su tamaño.
type Stamp = (SystemTime, u64);

fn stamp(path: &Path) -> Option<Stamp> {
    let meta = fs::metadata(path).ok()?;
    Some((meta.modified().ok()?, meta.len()))
}

/// Vigila un archivo de patch para la recarga en caliente.
///
/// Un cambio solo se aplica cuando la huella se mantiene igual en dos
/// comprobaciones seguidas: así no se lee un archivo que el editor aún está
/// escribiendo.
pub struct Watcher {
    path: PathBuf,
    applied: Option<Stamp>,
    pending: Option<Stamp>,
    last_check: Instant,
}

impl Watcher {
    /// Empieza a vigilar `path`; el contenido actual se da por ya cargado.
    pub fn new(path: PathBuf) -> Self {
        let applied = stamp(&path);
        Self { path, applied, pending: None, last_check: Instant::now() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Da por cargado el estado actual del archivo (p. ej. tras guardarlo uno mismo).
    pub fn mark_current(&mut self) {
        self.applied = stamp(&self.path);
        self.pending = None;
    }

    /// Se puede llamar en cada cuadro: solo mira el disco cada pocos milisegundos.
    /// Devuelve el patch recargado, o el motivo por el que no se pudo.
    pub fn poll(&mut self) -> Option<Result<Patch, String>> {
        if self.last_check.elapsed() < POLL_EVERY {
            return None;
        }
        self.last_check = Instant::now();
        self.check()
    }

    fn check(&mut self) -> Option<Result<Patch, String>> {
        let current = stamp(&self.path);
        if current == self.applied {
            self.pending = None;
            return None;
        }
        if current != self.pending {
            // Cambió: se espera a la siguiente comprobación para confirmar que se estabilizó.
            self.pending = current;
            return None;
        }
        self.applied = current;
        self.pending = None;
        // Si el archivo desapareció no se toca el sonido actual.
        current?;
        Some(load(&self.path))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use distrodaw_engine::ParamId;

    /// Directorio temporal propio de cada prueba, borrado al terminar.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new(name: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("distrodaw-test-{}-{name}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn save_then_load_round_trips() {
        let dir = TempDir::new("roundtrip");
        let path = dir.0.join("a.toml");
        let mut patch = Patch { name: "A".into(), ..Patch::default() };
        patch.params.set(ParamId::Release, 2.5);
        save(&path, &patch).unwrap();
        assert_eq!(load(&path).unwrap(), patch);
        assert!(!dir.0.join("a.toml.tmp").exists(), "no debe quedar el temporal");
    }

    #[test]
    fn load_errors_mention_the_file() {
        let dir = TempDir::new("errors");
        let missing = load(&dir.0.join("nope.toml")).unwrap_err();
        assert!(missing.starts_with("nope.toml:"));
        fs::write(dir.0.join("bad.toml"), "[envelope]\nsustain = 9\n").unwrap();
        let bad = load(&dir.0.join("bad.toml")).unwrap_err();
        assert!(bad.starts_with("bad.toml:") && bad.contains("sustain"), "{bad}");
    }

    #[test]
    fn list_returns_only_toml_files_sorted() {
        let dir = TempDir::new("list");
        for name in ["b.toml", "a.toml", "notes.txt", "c.toml.tmp"] {
            fs::write(dir.0.join(name), "").unwrap();
        }
        fs::create_dir(dir.0.join("sub.toml")).unwrap();
        let names: Vec<String> = list(&dir.0)
            .unwrap()
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["a.toml", "b.toml"]);
    }

    #[test]
    fn new_file_path_skips_existing_numbers() {
        let dir = TempDir::new("newfile");
        assert_eq!(new_file_path(&dir.0), dir.0.join("patch-001.toml"));
        fs::write(dir.0.join("patch-001.toml"), "").unwrap();
        fs::write(dir.0.join("patch-002.toml"), "").unwrap();
        assert_eq!(new_file_path(&dir.0), dir.0.join("patch-003.toml"));
    }

    #[test]
    fn watcher_waits_for_the_file_to_settle_before_reloading() {
        let dir = TempDir::new("watch");
        let path = dir.0.join("w.toml");
        fs::write(&path, "[output]\nvolume = 0.5\n").unwrap();
        let mut watcher = Watcher::new(path.clone());
        assert!(watcher.check().is_none(), "sin cambios no hay recarga");

        fs::write(&path, "[output]\nvolume = 0.25  \n").unwrap();
        assert!(watcher.check().is_none(), "primer aviso: aún se confirma");
        let patch = watcher.check().expect("segunda comprobación: se aplica").unwrap();
        assert_eq!(patch.params.get(ParamId::Volume), 0.25);
        assert!(watcher.check().is_none(), "ya aplicado");
    }

    #[test]
    fn watcher_reports_invalid_edits_once_and_keeps_going() {
        let dir = TempDir::new("watch-bad");
        let path = dir.0.join("w.toml");
        fs::write(&path, "").unwrap();
        let mut watcher = Watcher::new(path.clone());

        fs::write(&path, "[envelope]\nsustain = 5\n").unwrap();
        watcher.check();
        let err = watcher.check().unwrap().unwrap_err();
        assert!(err.contains("sustain"));
        assert!(watcher.check().is_none(), "el error no se repite");

        fs::write(&path, "[envelope]\nsustain = 0.5 \n").unwrap();
        watcher.check();
        assert!(watcher.check().unwrap().is_ok());
    }

    #[test]
    fn watcher_ignores_a_deleted_file_and_our_own_saves() {
        let dir = TempDir::new("watch-own");
        let path = dir.0.join("w.toml");
        save(&path, &Patch::default()).unwrap();
        let mut watcher = Watcher::new(path.clone());

        let mut patch = Patch::default();
        patch.params.set(ParamId::Volume, 0.9);
        save(&path, &patch).unwrap();
        watcher.mark_current();
        assert!(watcher.check().is_none() && watcher.check().is_none());

        fs::remove_file(&path).unwrap();
        watcher.check();
        assert!(watcher.check().is_none(), "sin archivo no se recarga nada");
    }

    #[test]
    fn every_example_patch_in_the_repo_is_valid() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../patches");
        let files = list(&dir).unwrap();
        assert!(files.len() >= 5, "faltan patches de ejemplo en {}", dir.display());
        for file in files {
            load(&file).unwrap_or_else(|e| panic!("patch de ejemplo inválido: {e}"));
        }
    }
}
