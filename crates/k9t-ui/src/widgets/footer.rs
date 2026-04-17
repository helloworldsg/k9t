use ratatui::{
    Frame,
    style::Modifier,
    text::{Line, Span},
    widgets::Paragraph,
};

use crate::theme::Theme;

/// Spinner characters for the sync indicator (rotates each tick)
const SPINNER_FRAMES: [char; 4] = ['◐', '◓', '◑', '◒'];

/// Renders the footer bar with contextual hints.
///
/// # Arguments
/// * `frame` - The frame to render to
/// * `area` - The area to render in
/// * `mode_name` - The current mode name to display
/// * `context_hints` - Mode-specific key hints
/// * `theme` - The theme to use
/// * `show_universal_hints` - Whether to show universal shortcuts (q, /, ?) - defaults to true
/// * `tick_count` - Current tick count for animating the sync indicator (0 = no indicator)
pub fn render_footer(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    mode_name: &str,
    context_hints: &[(&str, &str)],
    theme: &Theme,
    show_universal_hints: bool,
    tick_count: u64,
) {
    let mut spans = Vec::new();

    // Sync indicator (spinning circle) on the left when active
    if tick_count > 0 {
        let spinner_idx = (tick_count % (SPINNER_FRAMES.len() as u64)) as usize;
        let spinner = SPINNER_FRAMES[spinner_idx];
        spans.push(Span::styled(
            format!(" {} ", spinner),
            theme.fg_muted(),
        ));
    }

    for (key, label) in context_hints {
        if !spans.is_empty() {
            spans.push(Span::styled(" ", theme.fg_muted()));
        }
        spans.push(Span::styled(format!("[{key}]"), theme.fg_muted()));
        spans.push(Span::styled(*label, theme.fg_emphasis()));
    }

    // Universal hints only make sense in Normal mode
    if show_universal_hints {
        let universal = [("q", "uit"), ("/", "find"), ("?", "help")];

        for (key, label) in &universal {
            if !spans.is_empty() {
                spans.push(Span::styled(" ", theme.fg_muted()));
            }
            spans.push(Span::styled(format!("[{key}]"), theme.fg_muted()));
            spans.push(Span::styled(*label, theme.fg_emphasis()));
        }
    }

    if mode_name != "NORMAL" {
        let mode_span = Span::styled(
            format!(" {mode_name}"),
            theme.accent_primary().add_modifier(Modifier::BOLD),
        );
        spans.push(Span::styled(" ", theme.bg_base()));
        spans.push(mode_span);
    }

    let paragraph = Paragraph::new(Line::from(spans)).style(theme.bg_base());

    frame.render_widget(paragraph, area);
}

pub fn render_footer_default(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    mode_name: &str,
    theme: &Theme,
) {
    render_footer(frame, area, mode_name, &[], theme, true, 0);
}
