use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::app::{
    AppCommand, CountriesFilter, FocusPane, InputMode, KeyScope, KnockoutRoundFilter,
    MatchesFilter, Screen, StandingsFilter, StatsTab,
};

pub fn command_for_key(
    key: KeyEvent,
    mode: InputMode,
    scope: KeyScope,
    screen: Screen,
    focus: FocusPane,
) -> Option<AppCommand> {
    match mode {
        InputMode::Normal => normal_command(key, scope, screen, focus),
        InputMode::Search => search_command(key),
        InputMode::QuitConfirm => quit_confirm_command(key),
    }
}

fn normal_command(
    key: KeyEvent,
    scope: KeyScope,
    screen: Screen,
    focus: FocusPane,
) -> Option<AppCommand> {
    match key.code {
        KeyCode::Char('q') if !(scope == KeyScope::Screen && screen == Screen::Countries) => {
            Some(AppCommand::Quit)
        }
        KeyCode::Char('?') => Some(AppCommand::ToggleHelp),
        KeyCode::Char(' ') => Some(AppCommand::ToggleKeyScope),
        KeyCode::Char('/') => Some(AppCommand::StartSearch),
        KeyCode::Char('0') => Some(AppCommand::Navigate(Screen::Home)),
        KeyCode::Char('1') => Some(AppCommand::Navigate(Screen::Countries)),
        KeyCode::Char('2') => Some(AppCommand::Navigate(Screen::Matches)),
        KeyCode::Char('3') => Some(AppCommand::Navigate(Screen::Standings)),
        KeyCode::Char('4') => Some(AppCommand::Navigate(Screen::Knockouts)),
        KeyCode::Char('5') => Some(AppCommand::Navigate(Screen::Stats)),
        KeyCode::Char('6') => Some(AppCommand::Navigate(Screen::Facts)),
        KeyCode::Tab => Some(AppCommand::FocusNext),
        KeyCode::BackTab => Some(AppCommand::FocusPrevious),
        KeyCode::Enter => Some(AppCommand::OpenDetails),
        KeyCode::Esc => Some(AppCommand::CloseOverlay),
        _ => match scope {
            KeyScope::Global => focused_scroll_command(key, focus).or_else(|| global_command(key)),
            KeyScope::Screen => {
                screen_command(key, screen, focus).or_else(|| focused_scroll_command(key, focus))
            }
        },
    }
}

fn focused_scroll_command(key: KeyEvent, focus: FocusPane) -> Option<AppCommand> {
    if focus == FocusPane::None {
        return None;
    }

    match key.code {
        KeyCode::Up => Some(AppCommand::ScrollUp {
            max: 0,
            visible_lines: 0,
        }),
        KeyCode::Down => Some(AppCommand::ScrollDown {
            max: 0,
            visible_lines: 0,
        }),
        KeyCode::Left => Some(AppCommand::ScrollLeft { max: 0 }),
        KeyCode::Right => Some(AppCommand::ScrollRight { max: 0 }),
        _ => None,
    }
}

fn global_command(key: KeyEvent) -> Option<AppCommand> {
    match command_char(key)? {
        'q' => Some(AppCommand::Quit),
        'h' => Some(AppCommand::ToggleHelp),
        's' | 'r' => Some(AppCommand::Refresh),
        'f' => Some(AppCommand::ToggleFavoriteOnly),
        't' => Some(AppCommand::ToggleTimeMode),
        _ => None,
    }
}

fn screen_command(key: KeyEvent, screen: Screen, focus: FocusPane) -> Option<AppCommand> {
    let character = command_char(key)?;

    match screen {
        Screen::Countries => {
            if character == 'q' {
                return Some(AppCommand::StartSearch);
            }

            if character == '*' {
                return Some(AppCommand::ToggleSelectedCountryFavorite);
            }

            if character == 'o' {
                return Some(AppCommand::ToggleSort);
            }

            CountriesFilter::ALL
                .iter()
                .find(|filter| filter.shortcut() == character)
                .copied()
                .map(AppCommand::ToggleCountryFilter)
        }
        Screen::Matches => {
            if focus == FocusPane::Detail {
                return crate::app::TimelineFilter::SELECTABLE
                    .iter()
                    .find(|filter| filter.shortcut() == character)
                    .copied()
                    .map(AppCommand::ToggleTimelineFilter);
            }

            if let Some(filter) = MatchesFilter::ALL
                .iter()
                .find(|filter| filter.shortcut() == character)
                .copied()
            {
                return Some(AppCommand::SelectMatchesFilter(filter));
            }

            StandingsFilter::ALL
                .iter()
                .find(|group| group.shortcut() == character)
                .copied()
                .map(AppCommand::ToggleMatchGroup)
        }
        Screen::Standings => {
            if character == 'm' && focus == FocusPane::Content {
                return Some(AppCommand::OpenSelectedStandingGroupMatches);
            }

            if character == 'o' {
                return Some(AppCommand::ToggleSort);
            }

            StandingsFilter::ALL
                .iter()
                .find(|group| group.shortcut() == character)
                .copied()
                .map(AppCommand::ToggleStandingGroup)
        }
        Screen::Knockouts => KnockoutRoundFilter::ALL
            .iter()
            .find(|round| round.shortcut() == character)
            .copied()
            .map(AppCommand::ToggleKnockoutRound),
        Screen::Stats => StatsTab::ALL
            .iter()
            .find(|tab| tab.shortcut() == character)
            .copied()
            .map(AppCommand::SelectStatsTab),
        Screen::Home | Screen::Facts => None,
    }
}

fn search_command(key: KeyEvent) -> Option<AppCommand> {
    match key.code {
        KeyCode::Esc => Some(AppCommand::CloseOverlay),
        KeyCode::Enter => Some(AppCommand::SubmitSearch),
        KeyCode::Backspace => Some(AppCommand::SearchBackspace),
        KeyCode::Char(character)
            if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
        {
            Some(AppCommand::SearchInput(character))
        }
        _ => None,
    }
}

fn quit_confirm_command(key: KeyEvent) -> Option<AppCommand> {
    match key.code {
        KeyCode::Enter => Some(AppCommand::ConfirmQuit),
        KeyCode::Esc => Some(AppCommand::CloseOverlay),
        KeyCode::Char(character) => match character.to_ascii_lowercase() {
            'q' | 'y' => Some(AppCommand::ConfirmQuit),
            'n' => Some(AppCommand::CloseOverlay),
            _ => None,
        },
        _ => None,
    }
}

fn command_char(key: KeyEvent) -> Option<char> {
    if key.modifiers.contains(KeyModifiers::CONTROL) || key.modifiers.contains(KeyModifiers::ALT) {
        return None;
    }

    let KeyCode::Char(character) = key.code else {
        return None;
    };

    Some(character.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEventKind, KeyEventState};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
            kind: KeyEventKind::Press,
            state: KeyEventState::empty(),
        }
    }

    #[test]
    fn maps_global_navigation_keys() {
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('2')),
                InputMode::Normal,
                KeyScope::Global,
                Screen::Home,
                FocusPane::None
            ),
            Some(AppCommand::Navigate(Screen::Matches))
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('h')),
                InputMode::Normal,
                KeyScope::Global,
                Screen::Matches,
                FocusPane::None
            ),
            Some(AppCommand::ToggleHelp)
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('4')),
                InputMode::Normal,
                KeyScope::Global,
                Screen::Home,
                FocusPane::None
            ),
            Some(AppCommand::Navigate(Screen::Knockouts))
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('m')),
                InputMode::Normal,
                KeyScope::Global,
                Screen::Standings,
                FocusPane::None
            ),
            None
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('c')),
                InputMode::Normal,
                KeyScope::Global,
                Screen::Standings,
                FocusPane::Content
            ),
            None
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('m')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Standings,
                FocusPane::Content
            ),
            Some(AppCommand::OpenSelectedStandingGroupMatches)
        );
    }

    #[test]
    fn maps_search_text_keys_in_search_mode() {
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('m')),
                InputMode::Search,
                KeyScope::Global,
                Screen::Countries,
                FocusPane::None
            ),
            Some(AppCommand::SearchInput('m'))
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Enter),
                InputMode::Search,
                KeyScope::Global,
                Screen::Countries,
                FocusPane::None
            ),
            Some(AppCommand::SubmitSearch)
        );
    }

    #[test]
    fn maps_same_key_by_scope() {
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('s')),
                InputMode::Normal,
                KeyScope::Global,
                Screen::Standings,
                FocusPane::None
            ),
            Some(AppCommand::Refresh)
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('r')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Standings,
                FocusPane::None
            ),
            Some(AppCommand::ToggleStandingGroup(StandingsFilter::AllGroups))
        );
    }

    #[test]
    fn focused_panel_scroll_uses_arrow_keys_only() {
        assert_eq!(
            command_for_key(
                key(KeyCode::Down),
                InputMode::Normal,
                KeyScope::Global,
                Screen::Standings,
                FocusPane::Content
            ),
            Some(AppCommand::ScrollDown {
                max: 0,
                visible_lines: 0,
            })
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Up),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Countries,
                FocusPane::Detail
            ),
            Some(AppCommand::ScrollUp {
                max: 0,
                visible_lines: 0,
            })
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('s')),
                InputMode::Normal,
                KeyScope::Global,
                Screen::Standings,
                FocusPane::Content
            ),
            Some(AppCommand::Refresh)
        );
    }

    #[test]
    fn screen_shortcuts_take_precedence_over_focused_scroll() {
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('r')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Standings,
                FocusPane::Content
            ),
            Some(AppCommand::ToggleStandingGroup(StandingsFilter::AllGroups))
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('j')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Standings,
                FocusPane::Content
            ),
            Some(AppCommand::ToggleStandingGroup(StandingsFilter::GroupJ))
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Down),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Standings,
                FocusPane::Content
            ),
            Some(AppCommand::ScrollDown {
                max: 0,
                visible_lines: 0,
            })
        );
    }

    #[test]
    fn focused_panel_supports_arrow_scroll() {
        assert_eq!(
            command_for_key(
                key(KeyCode::Down),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Countries,
                FocusPane::Content
            ),
            Some(AppCommand::ScrollDown {
                max: 0,
                visible_lines: 0,
            })
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Right),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Knockouts,
                FocusPane::Content
            ),
            Some(AppCommand::ScrollRight { max: 0 })
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Left),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Knockouts,
                FocusPane::Content
            ),
            Some(AppCommand::ScrollLeft { max: 0 })
        );
    }

    #[test]
    fn maps_screen_sort_shortcut_without_case_sensitivity() {
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('o')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Countries,
                FocusPane::None
            ),
            Some(AppCommand::ToggleSort)
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('O')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Standings,
                FocusPane::None
            ),
            Some(AppCommand::ToggleSort)
        );
    }

    #[test]
    fn maps_ofc_to_f_after_sort_claims_o() {
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('f')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Countries,
                FocusPane::None
            ),
            Some(AppCommand::ToggleCountryFilter(CountriesFilter::Ofc))
        );
    }

    #[test]
    fn maps_country_screen_search_and_favorite_actions() {
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('q')),
                InputMode::Normal,
                KeyScope::Global,
                Screen::Countries,
                FocusPane::None
            ),
            Some(AppCommand::Quit)
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('q')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Countries,
                FocusPane::None
            ),
            Some(AppCommand::StartSearch)
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('*')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Countries,
                FocusPane::Content
            ),
            Some(AppCommand::ToggleSelectedCountryFavorite)
        );
    }

    #[test]
    fn maps_quit_confirmation_keys() {
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('y')),
                InputMode::QuitConfirm,
                KeyScope::Global,
                Screen::Countries,
                FocusPane::None
            ),
            Some(AppCommand::ConfirmQuit)
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('q')),
                InputMode::QuitConfirm,
                KeyScope::Global,
                Screen::Countries,
                FocusPane::None
            ),
            Some(AppCommand::ConfirmQuit)
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Enter),
                InputMode::QuitConfirm,
                KeyScope::Global,
                Screen::Countries,
                FocusPane::None
            ),
            Some(AppCommand::ConfirmQuit)
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('n')),
                InputMode::QuitConfirm,
                KeyScope::Global,
                Screen::Countries,
                FocusPane::None
            ),
            Some(AppCommand::CloseOverlay)
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Esc),
                InputMode::QuitConfirm,
                KeyScope::Global,
                Screen::Countries,
                FocusPane::None
            ),
            Some(AppCommand::CloseOverlay)
        );
    }

    #[test]
    fn maps_match_group_shortcuts() {
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('r')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Matches,
                FocusPane::Content
            ),
            Some(AppCommand::ToggleMatchGroup(StandingsFilter::AllGroups))
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('d')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Matches,
                FocusPane::Content
            ),
            Some(AppCommand::ToggleMatchGroup(StandingsFilter::GroupD))
        );
    }

    #[test]
    fn maps_match_date_shortcuts() {
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('p')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Matches,
                FocusPane::Content
            ),
            Some(AppCommand::SelectMatchesFilter(MatchesFilter::Past))
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('u')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Matches,
                FocusPane::Content
            ),
            Some(AppCommand::SelectMatchesFilter(MatchesFilter::Future))
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('s')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Matches,
                FocusPane::Content
            ),
            Some(AppCommand::SelectMatchesFilter(MatchesFilter::All))
        );
    }

    #[test]
    fn maps_match_detail_timeline_filter_shortcuts() {
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('g')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Matches,
                FocusPane::Detail
            ),
            Some(AppCommand::ToggleTimelineFilter(
                crate::app::TimelineFilter::Goals
            ))
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('s')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Matches,
                FocusPane::Detail
            ),
            Some(AppCommand::ToggleTimelineFilter(
                crate::app::TimelineFilter::Substitutions
            ))
        );
    }

    #[test]
    fn maps_knockout_round_shortcuts() {
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('r')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Knockouts,
                FocusPane::Content
            ),
            Some(AppCommand::ToggleKnockoutRound(
                KnockoutRoundFilter::RoundOf32
            ))
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('o')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Knockouts,
                FocusPane::Content
            ),
            Some(AppCommand::ToggleKnockoutRound(
                KnockoutRoundFilter::RoundOf16
            ))
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('u')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Knockouts,
                FocusPane::Content
            ),
            Some(AppCommand::ToggleKnockoutRound(
                KnockoutRoundFilter::QuarterFinal
            ))
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('s')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Knockouts,
                FocusPane::Content
            ),
            Some(AppCommand::ToggleKnockoutRound(
                KnockoutRoundFilter::SemiFinal
            ))
        );
        assert_eq!(
            command_for_key(
                key(KeyCode::Char('f')),
                InputMode::Normal,
                KeyScope::Screen,
                Screen::Knockouts,
                FocusPane::Content
            ),
            Some(AppCommand::ToggleKnockoutRound(KnockoutRoundFilter::Final))
        );
    }
}
