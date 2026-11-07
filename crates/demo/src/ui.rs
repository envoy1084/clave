//! The screen: a header, Alice and Bob side by side, and the narration bar with the keys that apply right now.

use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Paragraph, Wrap},
};
use tui_term::widget::{Cursor, PseudoTerminal};

use crate::{
    runner::{Runner, State},
    script::Who,
};

const ACCENT: Color = Color::Cyan;
const TYPING: Color = Color::Yellow;

/// Draws the frame and resizes the terminals to their panes.
pub(crate) fn draw(frame: &mut Frame, runner: &mut Runner, countdown: Option<u64>) {
    let [header, panes, narration] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(8),
        Constraint::Length(8),
    ])
    .areas(frame.area());
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)]).areas(panes);

    let total = runner.steps.len();
    let step = (runner.step + 1).min(total);
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " Clave ",
                Style::new()
                    .fg(Color::Black)
                    .bg(ACCENT)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("  a private 2-of-2 vault on Aztec"),
            Span::styled(
                format!("   step {step} of {total}"),
                Style::new().fg(Color::DarkGray),
            ),
        ])),
        header,
    );
    for (who, area) in [(Who::Alice, left), (Who::Bob, right)] {
        draw_pane(frame, runner, who, area);
    }
    draw_narration(frame, runner, narration, countdown);
}

fn draw_pane(frame: &mut Frame, runner: &mut Runner, who: Who, area: Rect) {
    let typing_here = (runner.state == State::Running && runner.busy_pane() == Some(who))
        || (runner.state == State::Finished && runner.focus == who);
    let pane = runner.pane(who);
    let (color, status) = if typing_here && pane.busy {
        (
            TYPING,
            Span::styled(
                " ⌨ your keys go here ",
                Style::new().fg(Color::Black).bg(TYPING),
            ),
        )
    } else if pane.busy {
        (ACCENT, Span::styled(" ● running ", Style::new().fg(ACCENT)))
    } else if typing_here {
        (
            TYPING,
            Span::styled(" ⌨ live shell ", Style::new().fg(Color::Black).bg(TYPING)),
        )
    } else {
        match pane.last {
            Some(0) => (
                Color::DarkGray,
                Span::styled(" ✓ ", Style::new().fg(Color::Green)),
            ),
            Some(code) => (
                Color::Red,
                Span::styled(format!(" ✗ exit {code} "), Style::new().fg(Color::Red)),
            ),
            None => (Color::DarkGray, Span::raw("")),
        }
    };
    let block = Block::new()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(color))
        .title(Line::from(Span::styled(
            format!(" {} ", pane.title),
            Style::new().add_modifier(Modifier::BOLD),
        )))
        .title(Line::from(status).right_aligned());
    let inner = block.inner(area);
    pane.resize(inner.height, inner.width);
    let Some(parser) = pane.screen() else { return };
    let cursor = Cursor::default().visibility(typing_here);
    frame.render_widget(
        PseudoTerminal::new(parser.screen())
            .block(block)
            .cursor(cursor),
        area,
    );
}

fn draw_narration(frame: &mut Frame, runner: &Runner, area: Rect, countdown: Option<u64>) {
    let dim = Style::new().fg(Color::DarkGray);
    let key = |k: &'static str| {
        Span::styled(
            format!(" {k} "),
            Style::new().fg(Color::Black).bg(Color::Gray),
        )
    };
    let mut lines: Vec<Line> = Vec::new();
    let hints: Vec<Span> = match &runner.state {
        State::Finished => {
            lines.push(Line::styled(
                "Done. Both shells are live for questions.",
                Style::new().add_modifier(Modifier::BOLD),
            ));
            lines.push(Line::styled(
                "Try: clave status · clave proposals --status all · clave vault peers · clave activity",
                dim,
            ));
            vec![
                key("Tab"),
                Span::raw(" switch shell   "),
                key("Ctrl-Q"),
                Span::raw(" quit"),
            ]
        }
        state => {
            if let Some(step) = runner.steps.get(runner.step) {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{}  ", runner.step + 1),
                        Style::new().fg(ACCENT).add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(step.title, Style::new().add_modifier(Modifier::BOLD)),
                ]));
                for text in step.lines {
                    lines.push(Line::styled(
                        format!("   {text}"),
                        Style::new().fg(Color::Gray),
                    ));
                }
            }
            match state {
                State::Ready if !runner.shells_ready() => {
                    vec![Span::styled("starting the shells…", dim)]
                }
                State::Ready => {
                    let mut hint = vec![key("Enter"), Span::raw(" run this step   ")];
                    if let Some(seconds) = countdown {
                        hint.push(Span::styled(
                            format!("runs by itself in {seconds}s   "),
                            dim,
                        ));
                    }
                    hint.extend([key("Ctrl-Q"), Span::raw(" quit")]);
                    hint
                }
                State::Running => match runner.busy_pane() {
                    Some(who) => vec![
                        Span::styled(
                            format!(
                                "Keys go to {}'s terminal  ",
                                runner.panes[who.index()].title
                            ),
                            Style::new().fg(TYPING),
                        ),
                        key("Tab"),
                        Span::raw(" switch   "),
                        key("Ctrl-Q"),
                        Span::raw(" quit"),
                    ],
                    None => vec![Span::styled(format!("{}…", runner.activity), dim)],
                },
                State::Failed(reason) => vec![
                    Span::styled(format!("{reason}   "), Style::new().fg(Color::Red)),
                    key("r"),
                    Span::raw(" retry step   "),
                    key("s"),
                    Span::raw(" skip   "),
                    key("Ctrl-Q"),
                    Span::raw(" quit"),
                ],
                State::Finished => Vec::new(),
            }
        }
    };
    while lines.len() < 4 {
        lines.push(Line::raw(""));
    }
    lines.push(Line::from(hints));
    frame.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::new()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::new().fg(Color::DarkGray))
                .title(" Narration "),
        ),
        area,
    );
}

/// What a key press means.
pub(crate) enum Input {
    Quit,
    Run,
    Retry,
    Skip,
    SwitchFocus,
    /// Bytes for the focused terminal.
    Forward(Vec<u8>),
    Ignore,
}

/// Maps a key: while a terminal is busy (or the story is finished) keys go to it; otherwise they drive the story.
pub(crate) fn input(runner: &Runner, key: KeyEvent) -> Input {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl && key.code == KeyCode::Char('q') {
        return Input::Quit;
    }
    if key.code == KeyCode::Tab {
        return Input::SwitchFocus;
    }
    // Between steps a background command (Alice's lobby waiting for Bob) may still run; Enter then means "next step",
    // never input for that command.
    let to_terminal = (runner.state == State::Running && runner.busy_pane().is_some())
        || runner.state == State::Finished;
    if to_terminal {
        return bytes(key).map_or(Input::Ignore, Input::Forward);
    }
    match (&runner.state, key.code) {
        (State::Ready, KeyCode::Enter) => Input::Run,
        (State::Failed(_), KeyCode::Char('r')) => Input::Retry,
        (State::Failed(_), KeyCode::Char('s')) => Input::Skip,
        _ => Input::Ignore,
    }
}

/// The bytes a terminal expects for a key.
fn bytes(key: KeyEvent) -> Option<Vec<u8>> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    Some(match key.code {
        KeyCode::Char(c) if ctrl && c.is_ascii_alphabetic() => {
            vec![(c.to_ascii_lowercase() as u8) & 0x1f]
        }
        KeyCode::Char(c) => c.to_string().into_bytes(),
        KeyCode::Enter => b"\r".to_vec(),
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => b"\x1b[A".to_vec(),
        KeyCode::Down => b"\x1b[B".to_vec(),
        KeyCode::Right => b"\x1b[C".to_vec(),
        KeyCode::Left => b"\x1b[D".to_vec(),
        _ => return None,
    })
}
