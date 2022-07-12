use std::{io, thread, sync::mpsc::channel, time::Duration};
use tui::{
    backend::Backend,
    backend::CrosstermBackend,
    widgets::{Block, Gauge, Borders},
    layout::{Layout, Constraint, Direction},
    style::{Style, Color, Modifier},
    Terminal
};
use crossterm::{
    event::{read as read_event, poll as poll_event, Event as InputEvent, KeyEvent, KeyModifiers, KeyCode, DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

extern crate clap;
use clap::{Arg, App};

const GREY:Color = Color::Rgb(42, 42, 42);
const MUSTARD_YELLOW:Color = Color::Rgb(0xff, 0xe5, 0);

struct TimerStage {
    name: String,
    period_s: u32,
    elapsed_s: u32
}

struct Timer {
    stages: Vec<TimerStage>,
    current_timer: usize,
    paused: bool
}

fn update_state(timer: &mut Timer) -> bool {
    let Timer{
        stages,
        current_timer,
        paused
    } = timer;

    if *current_timer >= stages.len() {
        return false;
    }

    if *paused {
        return true;
    }

    let t = &mut stages[*current_timer];
    t.elapsed_s += 1;

    if t.period_s - t.elapsed_s == 0 {
        *current_timer += 1;
    }

    true
}

fn update_display<B: Backend>(
    terminal: &mut Terminal<B>,
    timer: &Timer,
    warning_threshold: u32
) -> Result<(), io::Error>
{
    terminal.draw(|f| {
        let Timer{
            stages,
            current_timer,
            paused
        } = timer;

        let num_chunks: u16 = (stages.len() + (100 % stages.len())).try_into().unwrap();
        let chunk_height: u16 = 100 / num_chunks;
        let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            (0..num_chunks)
                .map(|_| Constraint::Percentage(chunk_height))
                .collect::<Vec<Constraint>>()
        )
        .split(f.size());

        for (i, timer) in stages.iter().enumerate() {
            // let style = if i == *current_timer { BOLD_GREEN } else { DIM };
            let timer_completion = 1f64 - (timer.period_s - timer.elapsed_s) as f64 / timer.period_s as f64;

            let progr_bar = Gauge::default()
            .block(
                Block::default()
                .title(timer.name.to_string())
                .borders(Borders::NONE)
            )
            .gauge_style(
                Style::default()
                .fg(
                    if i == *current_timer {
                        if warning_threshold > 0 
                        && timer.period_s - timer.elapsed_s <= warning_threshold {
                            MUSTARD_YELLOW
                        } else {
                            Color::White
                        }
                    } else {
                        GREY
                    }
                )
                // .bg(Color::Black)
                .add_modifier(Modifier::BOLD)
            )
            .ratio(timer_completion)
            .label(if *paused {
                "Paused".to_string()
            } else {
                format!("{}s / {}s", timer.period_s - timer.elapsed_s, timer.period_s)
            });
            f.render_widget(progr_bar, chunks[i]);
        }
    })?;

    Ok(())
}

fn parse_cl_args() -> (Vec<(String, u32)>, u32) {
    let arg_match = App::new("Staged Timer")
        .version("0.1.0")
        .author("Jan Hettenkofer")
        .about("Configurable multi-stage timer for film development or workouts")
        .arg(Arg::with_name("name")
            .help("Name of the timer stage")
            .long("name")
            .short('n')
            .value_name("TIMER_NAME")
            .takes_value(true)
            .action(clap::ArgAction::Append)
            .required(true)
        )
        .arg(Arg::with_name("time")
            .help("Duration of the timer stage")
            .long("time")
            .short('t')
            .value_name("TIME")
            .takes_value(true)
            .value_parser(clap::value_parser!(u32))
            .action(clap::ArgAction::Append)
            .required(true)
        )
        .arg(Arg::with_name("warn")
            .help("Highlight the countdown bar at N remaining seconds")
            .long("warn")
            .short('w')
            .value_name("N")
            .takes_value(true)
            .value_parser(clap::value_parser!(u32))
            .default_value("0")
        )
        .get_matches();

    let input_names = arg_match.get_many::<String>("name").unwrap();
    let input_times = arg_match.get_many::<u32>("time").unwrap();
    let input_warn = arg_match.get_one::<u32>("warn").unwrap();

    if input_times.len() != input_names.len() {
        println!("Cannot match unequal number of timers and names.");
        std::process::exit(1);
    }

    (input_names.into_iter().cloned().zip(input_times.into_iter().cloned()).collect(), *input_warn)
}

fn create_timer_list(names_and_times: &[(String, u32)]) -> Vec<TimerStage> {
    names_and_times.iter().map(
        |(name, time)| {
            TimerStage {name: name.to_string(), period_s: *time, elapsed_s: 0}
        }
    ).collect()
}

fn main() -> Result<(), io::Error> {
    // == Data setup ===========================================================
    let (names_and_times, warn) = parse_cl_args();

    let mut timer = Timer {
        current_timer: 0,
        stages: create_timer_list(&names_and_times),
        paused: false
    };

    // == TUI setup ============================================================

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // == Main loop ============================================================

    update_display(&mut terminal, &timer, warn)?;

    let (tick_tx, tick_rx) = channel();

    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(1));
            tick_tx.send("tick").unwrap();
        }
    });

    let mut keep_running = true;
    while keep_running {
        thread::sleep(Duration::from_millis(50));

        let _ = tick_rx.try_recv().map(|_| {
            keep_running = update_state(&mut timer);
            keep_running = match update_display(
                &mut terminal,
                &timer,
                warn
            ) {
                Ok(_) => keep_running,
                Err(_) => false
            };
        });

        if poll_event(Duration::from_millis(50))? {
            let event = read_event()?;
            match event {
                // EXIT with CTRL+C or ESC
                InputEvent::Key(KeyEvent{
                    modifiers,
                    code
                }) if code == KeyCode::Esc || (
                    code == KeyCode::Char('c')
                    && modifiers == KeyModifiers::CONTROL
                ) => break,

                // PAUSE timer with SPACE BAR
                InputEvent::Key(KeyEvent{
                    modifiers: KeyModifiers::NONE,
                    code: KeyCode::Char(' ')
                }) => {
                    timer.paused = !timer.paused;
                    update_display(
                        &mut terminal,
                        &timer,
                        warn
                    )?;
                },
                _ => {}
            }
        }
    }

    // == Restore terminal state ===============================================

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    Ok(())

}
