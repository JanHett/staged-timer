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
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

extern crate clap;
use clap::{Arg, App};
struct Timer {
    name: String,
    period_s: u32,
    elapsed_s: u32
}

fn update_timers(timers: &mut[Timer], current_timer: &mut usize) -> bool {
    if *current_timer >= timers.len() {
        return false;
    }

    let t = &mut timers[*current_timer];
    t.elapsed_s += 1;

    if t.period_s - t.elapsed_s == 0 {
        *current_timer += 1;
    }

    true
}

fn update_display<B: Backend>(
    terminal: &mut Terminal<B>,
    timers: &[Timer],
    current_timer: & usize) -> Result<(), io::Error>
{
    terminal.draw(|f| {
        let num_chunks = timers.len() + (timers.len() % 100);
        let individual_height: u16 = (100 / timers.len()).try_into().unwrap();
        let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            (0..num_chunks)
                .map(|_| Constraint::Percentage(individual_height))
                .collect::<Vec<Constraint>>()
        )
        .split(f.size());

        for (i, timer) in timers.iter().enumerate() {
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
                .fg( if i == *current_timer {Color::Green} else {Color::White} )
                // .bg(Color::Black)
                .add_modifier(if i == *current_timer {Modifier::BOLD} else {Modifier::DIM})
            )
            .ratio(timer_completion)
            .label(format!("{}s / {}s", timer.elapsed_s, timer.period_s));
           f.render_widget(progr_bar, chunks[i]);
        }
    })?;

    Ok(())
}

fn parse_cl_args() -> Vec<(String, u32)> {
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
        .get_matches();

    let input_times = arg_match.get_many::<u32>("time").unwrap();
    let input_names = arg_match.get_many::<String>("name").unwrap();

    if input_times.len() != input_names.len() {
        println!("Cannot match unequal number of timers and names.");
        std::process::exit(1);
    }

    input_names.into_iter().cloned().zip(input_times.into_iter().cloned()).collect()
}

fn create_timer_list(names_and_times: &[(String, u32)]) -> Vec<Timer> {
    let mut timers = Vec::new();
    for (input_n, input_t) in names_and_times {
        timers.push(
            Timer {name: input_n.to_string(), period_s: *input_t, elapsed_s: 0}
        );

        println!("Timer '{input_n}' set for {input_t}s");
    }

    timers
}

fn main() -> Result<(), io::Error> {
    // == Data setup ===========================================================
    let names_and_times = parse_cl_args();

    let mut current_timer = 0;
    let mut timers = create_timer_list(&names_and_times);

    // == TUI setup ============================================================

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // == Main loop ============================================================

    update_display(&mut terminal, &mut timers, &current_timer)?;

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
            keep_running = update_timers(&mut timers, &mut current_timer);
            keep_running = match update_display(&mut terminal, &mut timers, &current_timer) {
                Ok(_) => keep_running,
                Err(_) => false
            };
        });
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
