//! Temas de color. Un tema es un archivo TOML con los colores de cada papel de
//! la interfaz (título, etiquetas, valores...), editable sin recompilar.
//!
//! ```toml
//! name = "Dracula"
//!
//! [colors]
//! background = "#282a36"
//! foreground = "#f8f8f2"
//! title      = "#bd93f9"   # títulos de pantalla
//! label      = "#6272a4"   # etiquetas y textos secundarios
//! value      = "#50fa7b"   # valores y barras
//! accent     = "#f1fa8c"   # resaltes (cadena de señal)
//! ok         = "#50fa7b"   # medidor y estado correcto
//! warn       = "#ffb86c"   # medidor alto
//! alert      = "#ff5555"   # errores, medidor al máximo
//! ```
//!
//! Los temas necesitan un terminal de 24 bits (foot, kitty, alacritty, la
//! mayoría de los de escritorio). La consola del kernel solo tiene 16 colores:
//! allí se usa [`Palette::basic`], que ignora el tema.

use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;
use std::fmt;

/// Cuántos colores admite el terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMode {
    /// Los 16 colores básicos (consola del kernel, terminales sencillos).
    Basic,
    /// Color de 24 bits.
    TrueColor,
}

impl ColorMode {
    /// Decide según el valor de la variable de entorno `COLORTERM`.
    pub fn detect(colorterm: Option<&str>) -> ColorMode {
        match colorterm.map(str::to_ascii_lowercase).as_deref() {
            Some("truecolor") | Some("24bit") => ColorMode::TrueColor,
            _ => ColorMode::Basic,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

impl Rgb {
    /// Acepta `#rrggbb` o `rrggbb`.
    pub fn parse(text: &str) -> Result<Rgb, String> {
        let hex = text.strip_prefix('#').unwrap_or(text);
        let valid = hex.len() == 6 && hex.chars().all(|c| c.is_ascii_hexdigit());
        if !valid {
            return Err(format!("{text:?} no es un color #rrggbb"));
        }
        let byte = |i: usize| u8::from_str_radix(&hex[i..i + 2], 16).map_err(|e| e.to_string());
        Ok(Rgb(byte(0)?, byte(2)?, byte(4)?))
    }

    fn color(self) -> Color {
        Color::Rgb(self.0, self.1, self.2)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    pub name: String,
    pub background: Rgb,
    pub foreground: Rgb,
    pub title: Rgb,
    pub label: Rgb,
    pub value: Rgb,
    pub accent: Rgb,
    pub ok: Rgb,
    pub warn: Rgb,
    pub alert: Rgb,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeError(pub String);

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ThemeError {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, default)]
struct ThemeFile {
    name: String,
    colors: ColorsFile,
}

/// Los colores que falten toman los de Dracula.
#[derive(Deserialize)]
#[serde(deny_unknown_fields, default)]
struct ColorsFile {
    background: String,
    foreground: String,
    title: String,
    label: String,
    value: String,
    accent: String,
    ok: String,
    warn: String,
    alert: String,
}

impl Default for ColorsFile {
    fn default() -> Self {
        let s = |hex: &str| hex.to_string();
        Self {
            background: s("#282a36"),
            foreground: s("#f8f8f2"),
            title: s("#bd93f9"),
            label: s("#6272a4"),
            value: s("#50fa7b"),
            accent: s("#f1fa8c"),
            ok: s("#50fa7b"),
            warn: s("#ffb86c"),
            alert: s("#ff5555"),
        }
    }
}

impl Default for ThemeFile {
    fn default() -> Self {
        Self { name: "Dracula".into(), colors: ColorsFile::default() }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Theme::from_toml("").expect("el tema por defecto es válido")
    }
}

impl Theme {
    pub fn from_toml(text: &str) -> Result<Theme, ThemeError> {
        let file: ThemeFile =
            toml::from_str(text).map_err(|e| ThemeError(e.message().to_string()))?;
        let c = &file.colors;
        let parse = |field: &str, value: &str| {
            Rgb::parse(value).map_err(|e| ThemeError(format!("colors.{field}: {e}")))
        };
        Ok(Theme {
            name: file.name.clone(),
            background: parse("background", &c.background)?,
            foreground: parse("foreground", &c.foreground)?,
            title: parse("title", &c.title)?,
            label: parse("label", &c.label)?,
            value: parse("value", &c.value)?,
            accent: parse("accent", &c.accent)?,
            ok: parse("ok", &c.ok)?,
            warn: parse("warn", &c.warn)?,
            alert: parse("alert", &c.alert)?,
        })
    }
}

/// Colores que usa el dibujo, ya resueltos para un modo de color.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Palette {
    /// `None` deja el fondo y el texto por defecto del terminal.
    pub background: Option<Color>,
    pub foreground: Option<Color>,
    pub title: Color,
    pub label: Color,
    pub value: Color,
    pub accent: Color,
    pub ok: Color,
    pub warn: Color,
    pub alert: Color,
}

impl Palette {
    /// Solo los 16 colores básicos: se ve bien en la consola del kernel y por SSH.
    pub fn basic() -> Palette {
        Palette {
            background: None,
            foreground: None,
            title: Color::Cyan,
            label: Color::DarkGray,
            value: Color::Green,
            accent: Color::Yellow,
            ok: Color::Green,
            warn: Color::Yellow,
            alert: Color::Red,
        }
    }

    /// Colores RGB del tema; solo tiene sentido en un terminal de 24 bits.
    pub fn from_theme(theme: &Theme) -> Palette {
        Palette {
            background: Some(theme.background.color()),
            foreground: Some(theme.foreground.color()),
            title: theme.title.color(),
            label: theme.label.color(),
            value: theme.value.color(),
            accent: theme.accent.color(),
            ok: theme.ok.color(),
            warn: theme.warn.color(),
            alert: theme.alert.color(),
        }
    }

    /// Estilo base de toda la pantalla (fondo y texto del tema, si hay).
    pub fn base(&self) -> Style {
        let mut style = Style::new();
        if let Some(bg) = self.background {
            style = style.bg(bg);
        }
        if let Some(fg) = self.foreground {
            style = style.fg(fg);
        }
        style
    }

    pub fn title_style(&self) -> Style {
        Style::new().fg(self.title).add_modifier(Modifier::BOLD)
    }

    pub fn label_style(&self) -> Style {
        Style::new().fg(self.label)
    }

    pub fn value_style(&self) -> Style {
        Style::new().fg(self.value)
    }

    pub fn accent_style(&self) -> Style {
        Style::new().fg(self.accent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colorterm_decides_the_color_mode() {
        assert_eq!(ColorMode::detect(Some("truecolor")), ColorMode::TrueColor);
        assert_eq!(ColorMode::detect(Some("24bit")), ColorMode::TrueColor);
        assert_eq!(ColorMode::detect(Some("TrueColor")), ColorMode::TrueColor);
        assert_eq!(ColorMode::detect(Some("yes")), ColorMode::Basic);
        assert_eq!(ColorMode::detect(None), ColorMode::Basic);
    }

    #[test]
    fn hex_colors_parse_with_or_without_hash() {
        assert_eq!(Rgb::parse("#282a36"), Ok(Rgb(0x28, 0x2a, 0x36)));
        assert_eq!(Rgb::parse("FFB86C"), Ok(Rgb(0xff, 0xb8, 0x6c)));
        for bad in ["", "#fff", "#12345", "#1234567", "#gg0000", "rojo"] {
            assert!(Rgb::parse(bad).is_err(), "{bad:?} debería fallar");
        }
    }

    #[test]
    fn the_default_theme_is_dracula() {
        let t = Theme::default();
        assert_eq!(t.name, "Dracula");
        assert_eq!(t.background, Rgb(0x28, 0x2a, 0x36));
        assert_eq!(t.alert, Rgb(0xff, 0x55, 0x55));
    }

    #[test]
    fn a_partial_theme_keeps_defaults_for_the_rest() {
        let t = Theme::from_toml("name = \"Mío\"\n[colors]\nbackground = \"#000000\"\n").unwrap();
        assert_eq!(t.name, "Mío");
        assert_eq!(t.background, Rgb(0, 0, 0));
        assert_eq!(t.title, Theme::default().title);
    }

    #[test]
    fn typos_and_bad_colors_are_errors_that_name_the_field() {
        assert!(Theme::from_toml("[colors]\nbackgrund = \"#000000\"\n").is_err());
        assert!(Theme::from_toml("[tema]\nx = 1\n").is_err());
        let err = Theme::from_toml("[colors]\nalert = \"rojo\"\n").unwrap_err();
        assert!(err.to_string().contains("colors.alert"), "{err}");
    }

    #[test]
    fn basic_palette_uses_ansi_colors_and_leaves_the_background_alone() {
        let p = Palette::basic();
        assert_eq!(p.background, None);
        assert_eq!(p.base(), Style::new());
        assert_eq!(p.alert, Color::Red);
    }

    #[test]
    fn theme_palette_is_rgb_and_sets_the_background() {
        let p = Palette::from_theme(&Theme::default());
        assert_eq!(p.background, Some(Color::Rgb(0x28, 0x2a, 0x36)));
        assert_eq!(p.base().fg, Some(Color::Rgb(0xf8, 0xf8, 0xf2)));
        assert_eq!(p.title, Color::Rgb(0xbd, 0x93, 0xf9));
    }
}
