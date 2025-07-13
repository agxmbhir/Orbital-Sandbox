use std::{ error::Error, io, time::{ Duration, Instant } };

use crossterm::{
    event::{ self, Event as CEvent, KeyCode },
    execute,
    terminal::{ disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen },
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
    widgets::{ Block, Borders, Row, Table },
    layout::{ Constraint, Direction, Layout },
    style::{ Modifier, Style },
};

use crate::ticks::MultiTickAMM;

pub fn run_ui(mut amm: MultiTickAMM) -> Result<(), Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let tick_rate = Duration::from_millis(500);
    let mut last_tick = Instant::now();

    loop {
        terminal.draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(1)
                .constraints([Constraint::Percentage(80), Constraint::Percentage(20)].as_ref())
                .split(f.size());

            // Table of ticks
            let header = Row::new(vec!["idx", "plane c", "parallel", "state"]);
            let rows: Vec<Row> = amm.ticks
                .iter()
                .enumerate()
                .map(|(i, t)| {
                    let (par, _) = crate::sphere::decompose_reserves(&t.sphere_amm.reserves);
                    let state = if t.is_interior() {
                        "interior"
                    } else if t.is_boundary() {
                        "boundary"
                    } else {
                        "exterior"
                    };
                    Row::new(
                        vec![
                            i.to_string(),
                            format!("{:.2}", t.plane_constant),
                            format!("{:.2}", par),
                            state.to_string()
                        ]
                    )
                })
                .collect();

            let table = Table::new(rows)
                .header(header)
                .block(Block::default().borders(Borders::ALL).title("Ticks"))
                .widths(
                    &[
                        Constraint::Length(5),
                        Constraint::Length(10),
                        Constraint::Length(12),
                        Constraint::Length(10),
                    ]
                );
            f.render_widget(table, chunks[0]);

            // Footer info
            let footer = Block::default()
                .borders(Borders::ALL)
                .title("Controls: q quit | r refresh from file | any key to continue");
            f.render_widget(footer, chunks[1]);
        })?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or_else(|| Duration::from_secs(0));
        if crossterm::event::poll(timeout)? {
            if let CEvent::Key(key) = event::read()? {
                match key.code {
                    KeyCode::Char('q') => {
                        break;
                    }
                    KeyCode::Char('r') => {
                        amm = MultiTickAMM::load_state(amm.token_names.clone());
                    }
                    _ => {}
                }
            }
        }
        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}
