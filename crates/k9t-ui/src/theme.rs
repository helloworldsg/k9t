use std::path::Path;

use ratatui::style::{Color, Modifier, Style};

#[derive(Debug, Clone)]
pub struct Theme {
    pub name: String,
    pub fg_default: Color,
    pub fg_muted: Color,
    pub fg_emphasis: Color,
    pub bg_base: Color,
    pub bg_surface: Color,
    pub bg_overlay: Color,
    pub bg_selection: Color,
    pub accent_primary: Color,
    pub accent_secondary: Color,
    pub status_error: Color,
    pub status_warning: Color,
    pub status_success: Color,
    pub status_info: Color,
}

impl Theme {
    pub fn fg_default(&self) -> Style {
        Style::default().fg(self.fg_default)
    }

    pub fn fg_muted(&self) -> Style {
        Style::default().fg(self.fg_muted)
    }

    pub fn fg_emphasis(&self) -> Style {
        Style::default().fg(self.fg_emphasis)
    }

    pub fn bg_base(&self) -> Style {
        Style::default().bg(self.bg_base)
    }

    pub fn bg_surface(&self) -> Style {
        Style::default().bg(self.bg_surface)
    }

    pub fn bg_overlay(&self) -> Style {
        Style::default().bg(self.bg_overlay)
    }

    pub fn bg_selection(&self) -> Style {
        Style::default().bg(self.bg_selection)
    }

    pub fn accent_primary(&self) -> Style {
        Style::default().fg(self.accent_primary)
    }

    pub fn accent_secondary(&self) -> Style {
        Style::default().fg(self.accent_secondary)
    }

    pub fn status_error(&self) -> Style {
        Style::default().fg(self.status_error)
    }

    pub fn status_warning(&self) -> Style {
        Style::default().fg(self.status_warning)
    }

    pub fn status_success(&self) -> Style {
        Style::default().fg(self.status_success)
    }

    pub fn status_info(&self) -> Style {
        Style::default().fg(self.status_info)
    }

    pub fn title_style(&self) -> Style {
        Style::default()
            .fg(self.fg_emphasis)
            .add_modifier(Modifier::BOLD)
    }

    pub fn selected_style(&self) -> Style {
        Style::default().fg(self.fg_emphasis).bg(self.bg_selection)
    }

    pub fn status_style(&self, status: &str) -> Style {
        match status {
            "error" => self.status_error(),
            "warning" => self.status_warning(),
            "success" => self.status_success(),
            "info" => self.status_info(),
            _ => self.fg_default(),
        }
    }

    pub fn load_from_toml(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let value: toml::Value = content.parse()?;

        let theme_table = value
            .get("theme")
            .or_else(|| value.get("colors"))
            .ok_or_else(|| {
                anyhow::anyhow!("No [theme] or [colors] section in {}", path.display())
            })?;

        Ok(Self {
            name: path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("custom")
                .to_string(),
            fg_default: parse_color(theme_table, "fg_default")?,
            fg_muted: parse_color(theme_table, "fg_muted")?,
            fg_emphasis: parse_color(theme_table, "fg_emphasis")?,
            bg_base: parse_color(theme_table, "bg_base")?,
            bg_surface: parse_color(theme_table, "bg_surface")?,
            bg_overlay: parse_color(theme_table, "bg_overlay")?,
            bg_selection: parse_color(theme_table, "bg_selection")?,
            accent_primary: parse_color(theme_table, "accent_primary")?,
            accent_secondary: parse_color(theme_table, "accent_secondary")?,
            status_error: parse_color(theme_table, "status_error")?,
            status_warning: parse_color(theme_table, "status_warning")?,
            status_success: parse_color(theme_table, "status_success")?,
            status_info: parse_color(theme_table, "status_info")?,
        })
    }
}

impl Theme {
    pub fn tokyo_night() -> Self {
        Self {
            name: "Tokyo Night".to_string(),
            fg_default: Color::Rgb(192, 202, 245),
            fg_muted: Color::Rgb(86, 95, 137),
            fg_emphasis: Color::Rgb(224, 224, 224),
            bg_base: Color::Rgb(26, 27, 38),
            bg_surface: Color::Rgb(36, 40, 59),
            bg_overlay: Color::Rgb(65, 72, 104),
            bg_selection: Color::Rgb(54, 74, 130),
            accent_primary: Color::Rgb(122, 162, 247),
            accent_secondary: Color::Rgb(187, 154, 247),
            status_error: Color::Rgb(247, 118, 142),
            status_warning: Color::Rgb(224, 175, 104),
            status_success: Color::Rgb(158, 206, 106),
            status_info: Color::Rgb(125, 207, 255),
        }
    }

    pub fn nord() -> Self {
        Self {
            name: "Nord".to_string(),
            fg_default: Color::Rgb(216, 222, 233),
            fg_muted: Color::Rgb(76, 86, 106),
            fg_emphasis: Color::Rgb(236, 239, 244),
            bg_base: Color::Rgb(46, 52, 64),
            bg_surface: Color::Rgb(59, 66, 82),
            bg_overlay: Color::Rgb(67, 76, 94),
            bg_selection: Color::Rgb(94, 129, 172),
            accent_primary: Color::Rgb(136, 192, 208),
            accent_secondary: Color::Rgb(180, 142, 173),
            status_error: Color::Rgb(191, 97, 106),
            status_warning: Color::Rgb(235, 203, 139),
            status_success: Color::Rgb(163, 190, 140),
            status_info: Color::Rgb(136, 192, 208),
        }
    }

    pub fn dracula() -> Self {
        Self {
            name: "Dracula".to_string(),
            fg_default: Color::Rgb(248, 248, 242),
            fg_muted: Color::Rgb(98, 114, 164),
            fg_emphasis: Color::Rgb(248, 248, 242),
            bg_base: Color::Rgb(40, 42, 54),
            bg_surface: Color::Rgb(44, 46, 59),
            bg_overlay: Color::Rgb(68, 71, 90),
            bg_selection: Color::Rgb(68, 71, 90),
            accent_primary: Color::Rgb(189, 147, 249),
            accent_secondary: Color::Rgb(255, 121, 198),
            status_error: Color::Rgb(255, 85, 85),
            status_warning: Color::Rgb(241, 250, 140),
            status_success: Color::Rgb(80, 250, 123),
            status_info: Color::Rgb(98, 214, 246),
        }
    }

    pub fn gruvbox() -> Self {
        Self {
            name: "Gruvbox".to_string(),
            fg_default: Color::Rgb(213, 196, 154),
            fg_muted: Color::Rgb(102, 92, 84),
            fg_emphasis: Color::Rgb(235, 219, 178),
            bg_base: Color::Rgb(40, 40, 40),
            bg_surface: Color::Rgb(60, 56, 54),
            bg_overlay: Color::Rgb(80, 73, 69),
            bg_selection: Color::Rgb(102, 92, 84),
            accent_primary: Color::Rgb(254, 128, 25),
            accent_secondary: Color::Rgb(211, 134, 155),
            status_error: Color::Rgb(251, 73, 52),
            status_warning: Color::Rgb(250, 189, 47),
            status_success: Color::Rgb(184, 187, 38),
            status_info: Color::Rgb(131, 165, 152),
        }
    }

    pub fn catppuccin_mocha() -> Self {
        Self {
            name: "Catppuccin Mocha".to_string(),
            fg_default: Color::Rgb(205, 214, 244),
            fg_muted: Color::Rgb(108, 112, 134),
            fg_emphasis: Color::Rgb(245, 226, 201),
            bg_base: Color::Rgb(30, 30, 46),
            bg_surface: Color::Rgb(49, 50, 68),
            bg_overlay: Color::Rgb(69, 71, 90),
            bg_selection: Color::Rgb(88, 91, 112),
            accent_primary: Color::Rgb(203, 166, 247),
            accent_secondary: Color::Rgb(245, 194, 231),
            status_error: Color::Rgb(243, 139, 168),
            status_warning: Color::Rgb(249, 226, 175),
            status_success: Color::Rgb(166, 227, 161),
            status_info: Color::Rgb(137, 180, 250),
        }
    }

    pub fn monochrome() -> Self {
        Self {
            name: "Monochrome".to_string(),
            fg_default: Color::White,
            fg_muted: Color::DarkGray,
            fg_emphasis: Color::White,
            bg_base: Color::Black,
            bg_surface: Color::DarkGray,
            bg_overlay: Color::Gray,
            bg_selection: Color::Gray,
            accent_primary: Color::White,
            accent_secondary: Color::Gray,
            status_error: Color::White,
            status_warning: Color::Gray,
            status_success: Color::White,
            status_info: Color::Gray,
        }
    }

    pub fn all_themes() -> Vec<Self> {
        vec![
            Self::tokyo_night(),
            Self::nord(),
            Self::dracula(),
            Self::gruvbox(),
            Self::catppuccin_mocha(),
            Self::light(),
        ]
    }

    pub fn auto() -> Self {
        if std::env::var("NO_COLOR").is_ok() {
            Self::monochrome()
        } else if Self::terminal_is_light() {
            Self::light()
        } else {
            Self::tokyo_night()
        }
    }

    fn terminal_is_light() -> bool {
        std::env::var("COLOR_SCHEME")
            .or_else(|_| std::env::var("TERM_PROGRAM"))
            .map(|v| v.to_lowercase().contains("light"))
            .unwrap_or(false)
    }

    pub fn light() -> Self {
        Self {
            name: "Light".to_string(),
            fg_default: Color::Rgb(30, 30, 30),
            fg_muted: Color::Rgb(120, 120, 120),
            fg_emphasis: Color::Rgb(0, 0, 0),
            bg_base: Color::Rgb(255, 255, 255),
            bg_surface: Color::Rgb(245, 245, 245),
            bg_overlay: Color::Rgb(220, 220, 220),
            bg_selection: Color::Rgb(180, 200, 240),
            accent_primary: Color::Rgb(0, 100, 200),
            accent_secondary: Color::Rgb(140, 60, 180),
            status_error: Color::Rgb(200, 30, 30),
            status_warning: Color::Rgb(170, 120, 0),
            status_success: Color::Rgb(40, 140, 50),
            status_info: Color::Rgb(30, 100, 200),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::auto()
    }
}

fn parse_color(table: &toml::Value, key: &str) -> anyhow::Result<Color> {
    let val = table
        .get(key)
        .ok_or_else(|| anyhow::anyhow!("missing color key: {key}"))?;

    let s = val
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("color key '{key}' must be a string"))?;

    parse_hex_color(s)
}

fn parse_hex_color(s: &str) -> anyhow::Result<Color> {
    let s = s.trim().trim_start_matches('#');

    if s.len() != 6 {
        anyhow::bail!("invalid hex color '#{s}': expected 6 hex digits");
    }

    let r = u8::from_str_radix(&s[0..2], 16)?;
    let g = u8::from_str_radix(&s[2..4], 16)?;
    let b = u8::from_str_radix(&s[4..6], 16)?;

    Ok(Color::Rgb(r, g, b))
}
