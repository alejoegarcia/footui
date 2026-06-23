use ratatui::{Frame, layout::Rect, text::Line};

use crate::{app::App, ui::components};

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App) -> components::PanelMetrics {
    let header = vec![components::filter_line("No filters")];
    let lines = vec![
        Line::from("Fun facts placeholder"),
        Line::from(""),
        Line::from("Content source is still TBD in the plan."),
        Line::from(
            "Initial implementation can use a static data/fun_facts.json seed with citations.",
        ),
        Line::from("Future fields: title, body, tags, related team, related match, source URL."),
    ];

    components::render_screen_frame(
        frame,
        area,
        "Fun Facts",
        header,
        lines,
        components::content_focused(app),
        app.content_scroll(),
    )
}
