use ratatui::{Frame, layout::Rect, text::Line};

use crate::app::App;
use crate::{app::StatsTab, ui::components};

pub fn render(frame: &mut Frame<'_>, area: Rect, app: &App) -> components::PanelMetrics {
    let tab = app.stats_tab().label();
    let header = vec![components::filter_line(tab)];

    let body = match app.stats_tab() {
        StatsTab::All => "Summary cards and reliable leaders will land after cached data exists.",
        StatsTab::Records => {
            "Records need a static, cited seed source. No fake records will be shown."
        }
        StatsTab::Goals => "Top scorers will come from FIFA topseasonplayerstatistics.",
        StatsTab::Fouls => "Fouls are experimental until timeline event mapping is validated.",
        StatsTab::Passes => "Passes are unsupported until a reliable source is confirmed.",
    };

    let lines = vec![
        Line::from("Stats placeholder"),
        Line::from(""),
        components::shortcut_menu_line_for_scope(
            "Tabs: ",
            StatsTab::ALL.iter().map(|tab| tab.menu_label()),
            components::screen_scope_active(app),
        ),
        Line::from(body),
        Line::from(""),
        Line::from(
            "Unsupported categories will render explicit empty states instead of fake data.",
        ),
    ];

    components::render_screen_frame(
        frame,
        area,
        "Stats",
        header,
        lines,
        components::content_focused(app),
        app.content_scroll(),
    )
}
