use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::{App, KeyScope};

use super::components;

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App) -> components::PanelMetrics {
    let scope = app.key_scope();
    let scope_sample_line = if scope == KeyScope::Screen {
        "You found your Space key, nice!\n"
    } else {
        ""
    };

    let lines = vec![
        Line::from("Browse countries, matches, standings and knock-outs"),
        Line::from(Span::styled(
            "(stats and fun facts TBD)",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from("Press Space to switch hotkey focus between global and screen scope"),
        Line::from(Span::styled(
            scope_sample_line,
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(vec![Span::styled(
            "All hotkeys are highlighted in yellow",
            Style::default().fg(Color::Yellow),
        )]),
        Line::from(""),
        Line::from(Span::styled(
            "FIFA public web endpoints are the adapter behind a data-source trait.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "Yep, cyan, yellow and white like the Argentinian flag. Sue me",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let max_scroll = components::max_scroll(lines.len(), area);
    let scroll = app.content_scroll().min(max_scroll);
    let home = Paragraph::new(lines)
        .block(
            Block::default()
                .title(home_title(scroll, max_scroll))
                .borders(Borders::ALL)
                .border_style(components::focus_style(components::content_focused(app))),
        )
        .scroll((scroll, 0))
        .wrap(Wrap { trim: true });
    frame.render_widget(home, area);

    components::PanelMetrics {
        content_max_scroll: max_scroll,
        detail_max_scroll: 0,
        content_max_horizontal_scroll: 0,
        detail_max_horizontal_scroll: 0,
        content_visible_lines: area.height.saturating_sub(2),
        detail_visible_lines: 0,
    }
}

fn home_title(scroll: u16, max_scroll: u16) -> String {
    if max_scroll == 0 {
        return "Home".to_string();
    }

    match (scroll > 0, scroll < max_scroll) {
        (false, true) => "Home ↓ more".to_string(),
        (true, true) => "Home ↑↓ more".to_string(),
        (true, false) => "Home ↑ more".to_string(),
        (false, false) => "Home".to_string(),
    }
}
