use std::iter::zip;
use std::io;
use std::io::Write;
use std::sync::mpsc::channel;
use std::thread;
use std::time::Duration;

extern crate clap;
use clap::{Arg, App};

const CLEAR_LINE: &str = "\x1b[2K\r";
// const BOLD: &str = "\x1b[1m";
const BOLD_GREEN: &str = "\x1b[1;32m";
const DIM: &str = "\x1b[2m";
const RESET_STYLE: &str = "\x1b[0m";

struct Timer {
    period_s: u32,
    elapsed_s: u32
}

fn update_timers(timers: &mut[(String, Timer)], current_timer: &mut usize) -> bool {
    if *current_timer >= timers.len() {
        return false;
    }

    let t = &mut timers[*current_timer].1;
    t.elapsed_s += 1;

    if t.period_s - t.elapsed_s == 0 {
        *current_timer += 1;
    }

    true
}

fn update_display(timers: &[(String, Timer)], current_timer: & usize) {
    let mut timer_repr:Vec<String> = Vec::new();
    for (i, (name, timer)) in timers.iter().enumerate() {
        let style = if i == *current_timer { BOLD_GREEN } else { DIM };
        timer_repr.push(format!("{style}{name}: {}{RESET_STYLE}", timer.period_s - timer.elapsed_s));
    }

    print!("{CLEAR_LINE}{}", timer_repr.join(" | "));
    io::stdout().flush().unwrap();
}

fn main() {
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

    let mut timers: Vec<(String, Timer)> = Vec::new();
    let mut current_timer = 0;

    for (input_n, input_t) in zip(input_names, input_times) {
        timers.push((
            input_n.to_string(),
            Timer {period_s: *input_t, elapsed_s: 0}
        ));

        println!("Timer '{input_n}' set for {input_t}s");
    }

    update_display(&mut timers, &current_timer);

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
            update_display(&mut timers, &current_timer);
        });
    }

}
