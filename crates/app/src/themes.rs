//! Archivos de temas de color (`themes/*.toml`).

use distrodaw_tui::Theme;
use std::fs;
use std::path::{Path, PathBuf};

/// `nombre` se busca como `<dir>/<nombre>.toml`; si lleva `/` o termina en
/// `.toml` se toma como ruta tal cual.
pub fn resolve(name: &str, dir: &Path) -> PathBuf {
    if name.contains('/') || name.ends_with(".toml") {
        PathBuf::from(name)
    } else {
        dir.join(format!("{name}.toml"))
    }
}

/// Lee y valida un tema. El error ya incluye el nombre del archivo.
pub fn load(name: &str, dir: &Path) -> Result<Theme, String> {
    let path = resolve(name, dir);
    let shown = path.display();
    let text = fs::read_to_string(&path).map_err(|e| format!("tema {shown}: {e}"))?;
    Theme::from_toml(&text).map_err(|e| format!("tema {shown}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_and_paths_resolve_differently() {
        let dir = Path::new("themes");
        assert_eq!(resolve("nord", dir), PathBuf::from("themes/nord.toml"));
        assert_eq!(resolve("/tmp/mio.toml", dir), PathBuf::from("/tmp/mio.toml"));
        assert_eq!(resolve("mio.toml", dir), PathBuf::from("mio.toml"));
    }

    #[test]
    fn a_missing_theme_names_the_file_in_the_error() {
        let err = load("no-existe", Path::new("themes")).unwrap_err();
        assert!(err.contains("themes/no-existe.toml"), "{err}");
    }

    #[test]
    fn every_theme_in_the_repo_is_valid_and_distinct() {
        let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../themes");
        let mut names = Vec::new();
        for entry in fs::read_dir(&dir).unwrap().filter_map(Result::ok) {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "toml") {
                let stem = path.file_stem().unwrap().to_string_lossy().into_owned();
                let theme = load(&stem, &dir).unwrap_or_else(|e| panic!("tema inválido: {e}"));
                names.push(theme.name);
            }
        }
        names.sort();
        let before = names.len();
        names.dedup();
        assert!(before >= 4, "faltan temas de ejemplo");
        assert_eq!(names.len(), before, "nombres de tema repetidos");
    }
}
