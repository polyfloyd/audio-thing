extern crate badlog;
extern crate byteorder;
extern crate dft;
extern crate id3;
#[macro_use]
extern crate lazy_static;
#[macro_use]
extern crate log;
extern crate notify;
extern crate regex;
extern crate rusqlite;
extern crate sample;
extern crate xdg;
extern crate libflac_sys;
extern crate liblame_sys;
extern crate libpulse_sys;
use std::*;
use std::io::BufRead;

mod audio;
mod filter;
mod format;
mod library;
mod player;
mod pulse;

fn main() {
    if cfg!(release) {
        badlog::init_from_env("LOG");
    } else {
        badlog::init(Some("debug"));
    }

    let filename = env::args().nth(1)
        .map(path::PathBuf::from)
        .expect("$1 should be an audio file");

    let (dyn_input, _) = format::decode_file(&filename).unwrap();
    let mut pb = player::Playback::new(dyn_input, &player::output::pulse::Output{});
    pb.set_playstate(player::State::Playing);

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
