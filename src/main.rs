extern crate dft;
extern crate flac;
extern crate sample;
use std::*;
use std::io::BufRead;
use ::audio::*;

mod audio;
mod filter;
mod format;
mod player;
mod pulse;

fn main() {
    let filename = env::args().nth(1)
        .expect("$1 should be an audio file");

    let file = fs::File::open(filename).unwrap();
    let input = format::flac::Decoder::new(file).unwrap();

    let dyn_input = dyn::Seek::StereoI16(Box::from(input)).into();
    let mut pb = player::play(dyn_input, sync::Arc::new(player::output::pulse::Output{}));

    let stdin = io::stdin();
    for line in stdin.lock().lines() {
        match line.unwrap().as_ref() {
            "ps" => pb.set_playstate(player::State::Paused),
            "pl" => pb.set_playstate(player::State::Playing),
            "st" => {
                let duration = pb.duration_time()
                    .map(|d| format!("{:?}", d))
                    .unwrap_or("∞".to_string());
                println!("state:    {:?}", pb.playstate());
                println!("position: {:?}/{}", pb.position_time(), duration);
                println!("tempo:    {}", pb.tempo());
                println!("latency:  {:?}", pb.stream.latency().unwrap());
            },
            l => {
                if let Ok(r) = l.parse() {
                    pb.set_tempo(r);
                }
            },
        }
    }
}
