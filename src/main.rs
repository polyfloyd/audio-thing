extern crate dft;
extern crate libflac_sys;
extern crate libpulse_sys;
extern crate sample;
use std::*;
use std::io::BufRead;

mod audio;
mod filter;
mod format;
mod player;
mod pulse;

fn main() {
    let filename = env::args().nth(1)
        .expect("$1 should be an audio file");

    let dyn_input = format::flac::open(&filename).unwrap();
    let mut pb = player::play(dyn_input, &player::output::pulse::Output{});

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        match line.unwrap().as_ref() {
            "ps" => pb.set_playstate(player::State::Paused),
            "pl" => pb.set_playstate(player::State::Playing),
            "st" => {
                let duration = pb.duration_time()
                    .map(|d| format_duraton(&d))
                    .unwrap_or("∞".to_string());
                println!("state:    {:?}", pb.playstate());
                println!("position: {}/{}", format_duraton(&pb.position_time()), duration);
                println!("tempo:    {}", pb.tempo());
                println!("latency:  {}ns", pb.stream.latency().unwrap().subsec_nanos());
            },
            l if l.starts_with(":") => {
                if let Ok(t) = l[1..].parse() {
                    pb.seek_time(time::Duration::new(t, 0));
                }
            },
            l => {
                if let Ok(r) = l.parse() {
                    pb.set_tempo(r);
                }
            },
        }
    }
}

fn format_duraton(dur: &time::Duration) -> String {
    let secs = dur.as_secs();
    let nanos = dur.subsec_nanos();
    format!("{:02}:{:02}.{}", secs / 60, secs % 60, nanos / 1_000_000_000)
}
