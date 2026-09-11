use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, List, ListItem, Paragraph, Wrap,
    },
    Frame,
};

use crate::app::{App, FocusPanel, ALL_SNAP_LAYOUTS};

pub fn draw(f: &mut Frame, app: &mut App) {
    let size = f.area();

    // Base background and outer split: Header, Main, Footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Footer
        ])
        .split(size);

    draw_header(f, app, chunks[0]);
    draw_main(f, app, chunks[1]);
    draw_footer(f, app, chunks[2]);
}

fn draw_header(f: &mut Frame, app: &App, area: Rect) {
    let (daemon_status_span, active_profile_span, uptime_span, overlays_span) = match &app.status {
        Some(st) => {
            let status_span = Span::styled(
                " ● ONLINE ",
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            );
            let profile_span = Span::styled(
                format!(" [{}] ", st.active_profile),
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            );
            let uptime_span = Span::styled(
                format!(" Uptime: {}s ", st.uptime_secs),
                Style::default().fg(Color::DarkGray),
            );
            let overlays_span = Span::styled(
                format!(" Overlays: {} ", st.overlays_count),
                Style::default().fg(Color::Yellow),
            );
            (status_span, profile_span, uptime_span, overlays_span)
        }
        None => {
            let status_span = Span::styled(
                " ○ OFFLINE (Connecting...) ",
                Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
            );
            let profile_span = Span::styled(
                " [N/A] ",
                Style::default().fg(Color::DarkGray),
            );
            let uptime_span = Span::styled(
                " Uptime: - ",
                Style::default().fg(Color::DarkGray),
            );
            let overlays_span = Span::styled(
                " Overlays: 0 ",
                Style::default().fg(Color::DarkGray),
            );
            (status_span, profile_span, uptime_span, overlays_span)
        }
    };

    let title = Line::from(vec![
        Span::styled(
            " AETHERSHIFT CONSOLE ",
            Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD),
        ),
        Span::raw("│"),
        daemon_status_span,
        Span::raw("│"),
        Span::raw(" Active Profile: "),
        active_profile_span,
        Span::raw("│"),
        overlays_span,
        Span::raw("│"),
        uptime_span,
    ]);

    let header_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::Cyan));

    let header_p = Paragraph::new(title)
        .block(header_block)
        .alignment(Alignment::Center);

    f.render_widget(header_p, area);
}

fn draw_main(f: &mut Frame, app: &mut App, area: Rect) {
    let main_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(45), // Left Panel: Profile Browser
            Constraint::Percentage(55), // Right Panels: Snap Matrix & Stats
        ])
        .split(area);

    draw_left_panel(f, app, main_chunks[0]);
    draw_right_panel(f, app, main_chunks[1]);
}

fn draw_left_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let is_focused = app.focus == FocusPanel::Profiles;
    let border_color = if is_focused {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let items: Vec<ListItem> = app
        .profiles
        .iter()
        .map(|p| {
            let active_marker = if p.is_active {
                Span::styled(" [ACTIVE]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
            } else {
                Span::raw("")
            };

            let line = Line::from(vec![
                Span::styled(
                    format!("{:<14}", p.name),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" ({} binds)", p.bindings_count),
                    Style::default().fg(Color::Cyan),
                ),
                active_marker,
                Span::styled(
                    format!("  - {}", p.description),
                    Style::default().fg(Color::Gray),
                ),
            ]);
            ListItem::new(line)
        })
        .collect();

    let title = if is_focused {
        " Profiles [Focused] "
    } else {
        " Profiles "
    };

    let list_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    let list = List::new(items)
        .block(list_block)
        .highlight_symbol("> ")
        .highlight_style(
            Style::default()
                .fg(Color::Black)
                .bg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(list, area, &mut app.profiles_state);
}

fn draw_right_panel(f: &mut Frame, app: &App, area: Rect) {
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(50), // Right Top Panel: Snap Matrix
            Constraint::Percentage(50), // Right Bottom Panel: Stats & Recommendations
        ])
        .split(area);

    draw_snap_panel(f, app, right_chunks[0]);
    draw_stats_panel(f, app, right_chunks[1]);
}

fn draw_snap_panel(f: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focus == FocusPanel::Snaps;
    let border_color = if is_focused {
        Color::Yellow
    } else {
        Color::DarkGray
    };

    let title = if is_focused {
        " Snap Matrix (14 Layouts) [Focused] "
    } else {
        " Snap Matrix (14 Layouts) "
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(border_color));

    // Render snap layouts in a 2-column grid format
    let mut lines = Vec::new();
    let total = ALL_SNAP_LAYOUTS.len();
    let rows = (total + 1) / 2;

    for row in 0..rows {
        let left_idx = row * 2;
        let right_idx = left_idx + 1;

        let format_item = |idx: usize| -> (String, Style) {
            let layout = ALL_SNAP_LAYOUTS[idx];
            let name = layout.as_str();
            let is_selected = idx == app.selected_snap_index;

            if is_selected {
                (
                    format!("> {:<20}", name),
                    if is_focused {
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD)
                    },
                )
            } else {
                (
                    format!("  {:<20}", name),
                    Style::default().fg(Color::White),
                )
            }
        };

        let (left_text, left_style) = format_item(left_idx);
        let mut spans = vec![Span::styled(left_text, left_style), Span::raw("  ")];

        if right_idx < total {
            let (right_text, right_style) = format_item(right_idx);
            spans.push(Span::styled(right_text, right_style));
        }

        lines.push(Line::from(spans));
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .alignment(Alignment::Left);

    f.render_widget(paragraph, area);
}

fn draw_stats_panel(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" Stats & Recommendations ")
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Color::DarkGray));

    let mut lines = Vec::new();

    // Stats Section
    if let Some(st) = &app.stats {
        lines.push(Line::from(vec![
            Span::styled("Total Switches: ", Style::default().fg(Color::Cyan)),
            Span::styled(st.total_switches.to_string(), Style::default().add_modifier(Modifier::BOLD)),
            Span::raw("  │  "),
            Span::styled("Total Actions: ", Style::default().fg(Color::Cyan)),
            Span::styled(st.total_actions.to_string(), Style::default().add_modifier(Modifier::BOLD)),
        ]));

        if !st.action_counts.is_empty() {
            let mut actions: Vec<(&String, &u64)> = st.action_counts.iter().collect();
            actions.sort_by(|a, b| b.1.cmp(a.1));
            let top_3: Vec<String> = actions
                .iter()
                .take(3)
                .map(|(act, cnt)| format!("{}: {}", act, cnt))
                .collect();
            lines.push(Line::from(vec![
                Span::styled("Top Actions: ", Style::default().fg(Color::Yellow)),
                Span::raw(top_3.join(", ")),
            ]));
        }
    } else {
        lines.push(Line::from(Span::styled(
            "Waiting for daemon usage statistics...",
            Style::default().fg(Color::DarkGray),
        )));
    }

    lines.push(Line::raw(""));

    // Recommendations Section
    lines.push(Line::from(Span::styled(
        "💡 Suggestions:",
        Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
    )));

    if app.recommendations.is_empty() {
        lines.push(Line::from(Span::styled(
            "No recommendations yet. Use shortcuts to gather insights.",
            Style::default().fg(Color::DarkGray),
        )));
    } else {
        for rec in app.recommendations.iter().take(2) {
            lines.push(Line::from(vec![
                Span::styled(format!("• [{}] ", rec.suggestion_type), Style::default().fg(Color::Yellow)),
                Span::styled(&rec.title, Style::default().add_modifier(Modifier::BOLD)),
            ]));
            lines.push(Line::from(Span::styled(
                format!("  {}", rec.message),
                Style::default().fg(Color::Gray),
            )));
        }
    }

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: true });

    f.render_widget(paragraph, area);
}

fn draw_footer(f: &mut Frame, app: &App, area: Rect) {
    let mut spans = vec![
        Span::styled("[Tab]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Focus  "),
        Span::styled("[↑/↓/j/k]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Navigate  "),
        Span::styled("[Enter]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Switch  "),
        Span::styled("[c]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Cycle  "),
        Span::styled("[r]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Restore  "),
        Span::styled("[s]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Snap  "),
        Span::styled("[q/Esc]", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        Span::raw(" Quit"),
    ];

    if let Some((msg, _, is_error)) = &app.message {
        spans.push(Span::raw("  │ "));
        let style = if *is_error {
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
        };
        spans.push(Span::styled(format!("Status: {msg}"), style));
    }

    let footer_p = Paragraph::new(Line::from(spans))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Blue)),
        )
        .alignment(Alignment::Left);

    f.render_widget(footer_p, area);
}
