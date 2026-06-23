use std::cmp;

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::{App, FocusPane, KeyScope};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PanelMetrics {
    pub content_max_scroll: u16,
    pub detail_max_scroll: u16,
    pub content_max_horizontal_scroll: u16,
    pub detail_max_horizontal_scroll: u16,
    pub content_visible_lines: u16,
    pub detail_visible_lines: u16,
}

impl PanelMetrics {
    pub fn merge(self, other: Self) -> Self {
        Self {
            content_max_scroll: cmp::max(self.content_max_scroll, other.content_max_scroll),
            detail_max_scroll: cmp::max(self.detail_max_scroll, other.detail_max_scroll),
            content_max_horizontal_scroll: cmp::max(
                self.content_max_horizontal_scroll,
                other.content_max_horizontal_scroll,
            ),
            detail_max_horizontal_scroll: cmp::max(
                self.detail_max_horizontal_scroll,
                other.detail_max_horizontal_scroll,
            ),
            content_visible_lines: cmp::max(
                self.content_visible_lines,
                other.content_visible_lines,
            ),
            detail_visible_lines: cmp::max(self.detail_visible_lines, other.detail_visible_lines),
        }
    }
}

pub fn render_screen_frame(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    header_lines: Vec<Line<'_>>,
    lines: Vec<Line<'_>>,
    focused: bool,
    scroll: u16,
) -> PanelMetrics {
    render_screen_frame_inner(
        frame,
        area,
        title,
        header_lines,
        lines,
        focused,
        scroll,
        0,
        true,
        false,
    )
}

pub fn render_screen_frame_unwrapped(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    header_lines: Vec<Line<'_>>,
    lines: Vec<Line<'_>>,
    focused: bool,
    scroll: u16,
) -> PanelMetrics {
    render_screen_frame_inner(
        frame,
        area,
        title,
        header_lines,
        lines,
        focused,
        scroll,
        0,
        false,
        false,
    )
}

pub fn render_screen_frame_unwrapped_scrolled_with_sticky(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    header_lines: Vec<Line<'_>>,
    sticky_lines: Vec<Line<'_>>,
    lines: Vec<Line<'_>>,
    focused: bool,
    scroll: u16,
    horizontal_scroll: u16,
) -> PanelMetrics {
    let header_height = header_lines.len() as u16 + 2;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(header_height), Constraint::Min(4)])
        .split(area);

    let tabs =
        Paragraph::new(header_lines).block(Block::default().title(title).borders(Borders::ALL));
    frame.render_widget(tabs, chunks[0]);

    let line_count = lines.len();
    let content_area = Block::default().borders(Borders::ALL).inner(chunks[1]);

    let sticky_height = sticky_lines.len().min(content_area.height as usize) as u16;
    let body_height = content_area.height.saturating_sub(sticky_height);
    let content_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(sticky_height), Constraint::Min(0)])
        .split(content_area);
    let sticky_area = content_chunks[0];
    let body_area = content_chunks[1];

    let max_scroll = max_scroll_for_visible(line_count, body_height);
    let scroll = scroll.min(max_scroll);
    let max_horizontal_scroll =
        max_horizontal_scroll_for_areas(&sticky_lines, &lines, content_area);
    let horizontal_scroll = horizontal_scroll.min(max_horizontal_scroll);
    let content_block = Block::default()
        .title(scroll_title_2d(
            "Content",
            scroll,
            max_scroll,
            horizontal_scroll,
            max_horizontal_scroll,
        ))
        .borders(Borders::ALL)
        .border_style(focus_style(focused));
    frame.render_widget(content_block, chunks[1]);

    if sticky_height > 0 {
        frame.render_widget(
            Paragraph::new(sticky_lines).scroll((0, horizontal_scroll)),
            sticky_area,
        );
    }
    frame.render_widget(
        Paragraph::new(lines).scroll((scroll, horizontal_scroll)),
        body_area,
    );

    PanelMetrics {
        content_max_scroll: max_scroll,
        detail_max_scroll: 0,
        content_max_horizontal_scroll: max_horizontal_scroll,
        detail_max_horizontal_scroll: 0,
        content_visible_lines: body_height,
        detail_visible_lines: 0,
    }
}

fn render_screen_frame_inner(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    header_lines: Vec<Line<'_>>,
    lines: Vec<Line<'_>>,
    focused: bool,
    scroll: u16,
    horizontal_scroll: u16,
    wrap: bool,
    horizontal: bool,
) -> PanelMetrics {
    let header_height = header_lines.len() as u16 + 2;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(header_height), Constraint::Min(4)])
        .split(area);

    let tabs =
        Paragraph::new(header_lines).block(Block::default().title(title).borders(Borders::ALL));
    frame.render_widget(tabs, chunks[0]);

    let line_count = lines.len();
    let max_scroll = max_scroll(line_count, chunks[1]);
    let scroll = scroll.min(max_scroll);
    let max_horizontal_scroll = if horizontal {
        max_horizontal_scroll(&lines, chunks[1])
    } else {
        0
    };
    let horizontal_scroll = horizontal_scroll.min(max_horizontal_scroll);
    let title = scroll_title_2d(
        "Content",
        scroll,
        max_scroll,
        horizontal_scroll,
        max_horizontal_scroll,
    );
    let body = Paragraph::new(lines)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(focus_style(focused)),
        )
        .scroll((scroll, horizontal_scroll));
    let body = if wrap {
        body.wrap(Wrap { trim: true })
    } else {
        body
    };
    frame.render_widget(body, chunks[1]);

    PanelMetrics {
        content_max_scroll: max_scroll,
        detail_max_scroll: 0,
        content_max_horizontal_scroll: max_horizontal_scroll,
        detail_max_horizontal_scroll: 0,
        content_visible_lines: chunks[1].height.saturating_sub(2),
        detail_visible_lines: 0,
    }
}

pub fn filter_line(value: impl Into<String>) -> Line<'static> {
    label_value_line("Filter: ", value)
}

pub fn sort_line_for_scope(
    subject: &'static str,
    value: impl Into<String>,
    active: bool,
) -> Line<'static> {
    let mut spans = shortcut_spans_for_scope("S[o]rt ", active);
    spans.push(Span::styled(subject, Style::default().fg(Color::DarkGray)));
    spans.push(Span::styled(": ", Style::default().fg(Color::DarkGray)));
    spans.push(Span::styled(value.into(), Style::default().fg(Color::Cyan)));
    Line::from(spans)
}

pub fn shortcut_menu_line_for_scope(
    prefix: &'static str,
    labels: impl IntoIterator<Item = &'static str>,
    active: bool,
) -> Line<'static> {
    let mut spans = vec![Span::styled(prefix, Style::default().fg(Color::DarkGray))];

    for (index, label) in labels.into_iter().enumerate() {
        if index > 0 {
            spans.push(Span::styled(" - ", Style::default().fg(Color::DarkGray)));
        }

        spans.extend(shortcut_spans_for_scope(label, active));
    }

    Line::from(spans)
}

pub fn label_value_line(label: &'static str, value: impl Into<String>) -> Line<'static> {
    Line::from(vec![
        Span::styled(label, Style::default().fg(Color::DarkGray)),
        Span::styled(value.into(), Style::default().fg(Color::Cyan)),
    ])
}

pub fn shortcut_spans_for_scope(label: &'static str, active: bool) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut normal = String::new();
    let mut chars = label.chars().peekable();
    let normal_style = if active {
        Style::default().fg(Color::Gray)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let shortcut_style = if active {
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };

    while let Some(character) = chars.next() {
        if character == '[' {
            if !normal.is_empty() {
                spans.push(Span::styled(std::mem::take(&mut normal), normal_style));
            }

            let mut shortcut = String::new();
            while let Some(next) = chars.next() {
                if next == ']' {
                    break;
                }
                shortcut.push(next);
            }

            if !shortcut.is_empty() {
                spans.push(Span::styled(shortcut, shortcut_style));
            }
        } else {
            normal.push(character);
        }
    }

    if !normal.is_empty() {
        spans.push(Span::styled(normal, normal_style));
    }

    spans
}

pub fn screen_scope_active(app: &App) -> bool {
    app.key_scope() == KeyScope::Screen
}

fn global_scope_active(app: &App) -> bool {
    app.key_scope() == KeyScope::Global
}

pub fn render_detail_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    app: &App,
    title: &str,
    focused: bool,
) -> PanelMetrics {
    let state = if app.detail_open() {
        "Detail placeholder is open."
    } else {
        "Press Enter to open the selected row details."
    };

    let lines = vec![
        Line::from(state),
        Line::from(""),
        Line::from("Data-backed fields planned here:"),
        Line::from("- selected row identity"),
        Line::from("- source payload summary"),
        Line::from("- screen-specific related rows"),
    ];
    let line_count = lines.len();
    let max_scroll = max_scroll(line_count, area);
    let scroll = app.detail_scroll().min(max_scroll);
    let title = scroll_title(title, scroll, max_scroll);

    let detail = Paragraph::new(lines)
        .block(
            Block::default()
                .title(title)
                .borders(Borders::ALL)
                .border_style(focus_style(focused)),
        )
        .scroll((scroll, 0))
        .style(Style::default().fg(Color::Gray))
        .wrap(Wrap { trim: true });
    frame.render_widget(detail, area);

    PanelMetrics {
        content_max_scroll: 0,
        detail_max_scroll: max_scroll,
        content_max_horizontal_scroll: 0,
        detail_max_horizontal_scroll: 0,
        content_visible_lines: 0,
        detail_visible_lines: area.height.saturating_sub(2),
    }
}

pub fn render_state_panel(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let lines = state_lines(app);
    let fg = if app.key_scope() == KeyScope::Global {
        Color::Cyan
    } else {
        Color::White
    };
    let panel = Paragraph::new(lines)
        .block(
            Block::default()
                .title("State")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(fg)),
        )
        .wrap(Wrap { trim: true });
    frame.render_widget(panel, area);
}

pub fn render_compact_status_bar(frame: &mut Frame<'_>, area: Rect, app: &App) {
    let line = compact_status_line(app);
    let status_bar = Paragraph::new(line).style(Style::default().bg(Color::Black));
    frame.render_widget(status_bar, area);
}

fn state_lines(app: &App) -> Vec<Line<'static>> {
    let status = app.source_status();
    let last_updated = app.last_sync_label();
    let favorite_state = if app.favorite_only() { "on" } else { "off" };
    let global_active = global_scope_active(app);

    vec![
        label_line("state", status.state.label()),
        label_line("scope", app.key_scope().label()),
        label_line("focus", app.focus_pane().label()),
        Line::from(""),
        shortcut_value_line("last [s]ync", last_updated, global_active),
        shortcut_value_line("[t]ime", app.time_mode().label(), global_active),
        shortcut_value_line("[f]avorites", favorite_state, global_active),
        Line::from(""),
        label_line("msg", ""),
        Line::from(format!("\n{}", app.message().to_string())),
    ]
}

fn compact_status_line(app: &App) -> Line<'static> {
    let status = app.source_status();
    let last_updated = app.last_sync_label();
    let favorite_state = if app.favorite_only() { "on" } else { "off" };

    let mut spans = vec![
        Span::styled(
            format!(" {} ", status.state.label()),
            Style::default()
                .fg(status_color(app))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("last "),
    ];
    let global_active = global_scope_active(app);
    spans.extend(shortcut_spans_for_scope("[s]ync", global_active));
    spans.push(Span::raw(format!(": {last_updated} | ")));
    spans.extend(shortcut_spans_for_scope("[t]ime", global_active));
    spans.push(Span::raw(format!(":{} | ", app.time_mode().label())));
    spans.extend(shortcut_spans_for_scope("[f]avorites", global_active));
    spans.push(Span::raw(format!(
        ":{} | scope:{} | focus:{} | ",
        favorite_state,
        app.key_scope().label(),
        app.focus_pane().label(),
    )));
    spans.push(Span::styled(
        app.message().to_string(),
        Style::default().fg(Color::White),
    ));

    Line::from(spans)
}

fn label_line(label: &'static str, value: impl Into<String>) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{label}: "), Style::default().fg(Color::DarkGray)),
        Span::styled(value.into(), Style::default().fg(Color::Gray)),
    ])
}

fn shortcut_value_line(
    label: &'static str,
    value: impl Into<String>,
    active: bool,
) -> Line<'static> {
    let mut spans = shortcut_spans_for_scope(label, active);
    spans.push(Span::styled(": ", Style::default().fg(Color::DarkGray)));
    spans.push(Span::styled(value.into(), Style::default().fg(Color::Gray)));
    Line::from(spans)
}

pub fn focus_style(focused: bool) -> Style {
    if focused {
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    }
}

pub fn scroll_title(base: &str, scroll: u16, max_scroll: u16) -> String {
    if max_scroll == 0 {
        return base.to_string();
    }

    let hint = match (scroll > 0, scroll < max_scroll) {
        (false, true) => "↓ more",
        (true, true) => "↑↓ more",
        (true, false) => "↑ more",
        (false, false) => "",
    };

    if hint.is_empty() {
        base.to_string()
    } else {
        format!("{base} {hint}")
    }
}

fn scroll_title_2d(
    base: &str,
    vertical: u16,
    max_vertical: u16,
    horizontal: u16,
    max_horizontal: u16,
) -> String {
    let vertical_hint = match (vertical > 0, vertical < max_vertical) {
        (false, true) => "↓",
        (true, true) => "↑↓",
        (true, false) => "↑",
        (false, false) => "",
    };
    let horizontal_hint = match (horizontal > 0, horizontal < max_horizontal) {
        (false, true) => "→",
        (true, true) => "←→",
        (true, false) => "←",
        (false, false) => "",
    };
    let hint = [vertical_hint, horizontal_hint]
        .into_iter()
        .filter(|hint| !hint.is_empty())
        .collect::<Vec<_>>()
        .join("");

    if hint.is_empty() {
        base.to_string()
    } else {
        format!("{base} {hint} more")
    }
}

pub fn max_scroll(line_count: usize, area: Rect) -> u16 {
    let visible_lines = area.height.saturating_sub(2) as usize;
    max_scroll_for_visible(line_count, visible_lines as u16)
}

fn max_scroll_for_visible(line_count: usize, visible_lines: u16) -> u16 {
    let visible_lines = visible_lines as usize;
    line_count
        .saturating_sub(visible_lines)
        .min(u16::MAX as usize) as u16
}

fn max_horizontal_scroll_for_areas(
    sticky_lines: &[Line<'_>],
    lines: &[Line<'_>],
    area: Rect,
) -> u16 {
    let visible_columns = area.width as usize;
    sticky_lines
        .iter()
        .chain(lines.iter())
        .map(line_width)
        .max()
        .unwrap_or(0)
        .saturating_sub(visible_columns)
        .min(u16::MAX as usize) as u16
}

fn max_horizontal_scroll(lines: &[Line<'_>], area: Rect) -> u16 {
    let visible_columns = area.width.saturating_sub(2) as usize;
    lines
        .iter()
        .map(line_width)
        .max()
        .unwrap_or(0)
        .saturating_sub(visible_columns)
        .min(u16::MAX as usize) as u16
}

fn line_width(line: &Line<'_>) -> usize {
    line.spans
        .iter()
        .map(|span| span.content.chars().count())
        .sum()
}

pub fn content_focused(app: &App) -> bool {
    app.focus_pane() == FocusPane::Content
}

pub fn detail_focused(app: &App) -> bool {
    app.focus_pane() == FocusPane::Detail
}

fn status_color(app: &App) -> Color {
    match app.source_status().state {
        crate::app::SourceState::Empty => Color::DarkGray,
        crate::app::SourceState::Cached => Color::Green,
        crate::app::SourceState::Refreshing => Color::Yellow,
        crate::app::SourceState::Offline => Color::Magenta,
        crate::app::SourceState::Error => Color::Red,
    }
}
