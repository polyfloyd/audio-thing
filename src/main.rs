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
use ::library::Library;

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

    let fs = library::fs::Filesystem::new(path::Path::new("testdata")).unwrap();

    let player = player::Player::new(Box::new(player::output::pulse::Output{}));

    let mut managed_id = env::args().nth(1)
        .map(|filename| {
            let mut p = player.lock().unwrap();
            let path = path::PathBuf::from(filename);
            let track = library::fs::track_from_path(&path).unwrap();
            p.queue.push(sync::Arc::new(library::Audio::Track(track)));
            let (id, _) = p.play_next_from_queue().unwrap().unwrap();
            id
        });

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    while let Some(Ok(line)) = lines.next() {
        let mut p = player.lock().unwrap();
        match line.as_ref() {
            "list" => {
                let tracks: Vec<_> = fs.tracks().unwrap()
                    .collect();
                for (i, track) in tracks.iter().enumerate() {
                    println!("{}: {} - {}", i, track.artists().get(0).unwrap_or(&"?".to_string()), track.title());
                }
                let track = lines.next().unwrap()
                    .unwrap_or("q".to_string())
                    .parse()
                    .ok()
                    .and_then(|index| tracks.into_iter().nth(index));
                if let Some(track) = track {
                    p.queue.push(sync::Arc::new(library::Audio::Track(track)));
                    managed_id.as_ref()
                        .and_then(|id| p.playing.get_mut(id))
                        .map(|&mut (_, ref mut pb, _)| pb.set_state(player::State::Stopped));
                    let i = p.queue.len() - 1;
                    let (id, _) = p.play_from_queue(i).unwrap().unwrap();
                    managed_id = Some(id);
                }
            },
            "prev" => {
                managed_id = p.play_previous_from_queue()
                    .unwrap()
                    .map(|t| t.0);
            },
            "next" => {
                managed_id = p.play_next_from_queue()
                    .unwrap()
                    .map(|t| t.0);
            },

            "pause" => {
                if let Some(&mut (_, ref mut pb, _)) = managed_id.as_ref().and_then(|id| p.playing.get_mut(id)) {
                    pb.set_state(player::State::Paused);
                }
            },
            "play" => {
                if let Some(&mut (_, ref mut pb, _)) = managed_id.as_ref().and_then(|id| p.playing.get_mut(id)) {
                    pb.set_state(player::State::Playing);
                }
            },
            "stop" => {
                if let Some(&mut (_, ref mut pb, _)) = managed_id.as_ref().and_then(|id| p.playing.get_mut(id)) {
                    pb.set_state(player::State::Stopped);
                }
            },
            "info" => {
                fn print_info<T: library::TrackInfo + ?Sized>(info: &T) {
                    println!("meta:");
                    println!("  title:   {}", info.title());
                    println!("  artists: {}", info.artists()
                             .get(0)
                             .unwrap_or(&"?".to_string()));
                    println!("  rating:  {}", info.rating()
                             .map(|r| format!("{}", r))
                             .unwrap_or("-".to_string()));
                }
                if let Some(&mut (ref audio, ref mut pb, ref info)) = managed_id.as_ref().and_then(|id| p.playing.get_mut(id)) {
                    let duration = pb.duration_time()
                        .map(|d| format_duraton(&d))
                        .unwrap_or("∞".to_string());
                    info.as_ref()
                        .map(|i| print_info(i.as_ref()))
                        .or_else(|| audio.track().map(print_info));
                    println!("state:    {:?}", pb.state());
                    println!("position: {}/{}", format_duraton(&pb.position_time()), duration);
                    println!("tempo:    {}", pb.tempo());
                    println!("latency:  {}ns", pb.stream.latency().unwrap().subsec_nanos());
                }
            },

            "queue" => {
                for (i, audio) in p.queue.iter().enumerate() {
                    if p.queue_cursor == Some(i) {
                        print!("-> ");
                    } else {
                        print!("   ");
                    }
                    match audio.as_ref() {
                        &library::Audio::Track(ref track) => {
                            println!("{} - {}", track.artists().get(0).unwrap_or(&"?".to_string()), track.title());
                        },
                        &library::Audio::Stream(_) => unimplemented!(),
                    };
                }
            },

            l if l.starts_with("j") => {
                if let Ok(i) = l[1..].parse() {
                    managed_id = p.play_from_queue(i)
                        .unwrap()
                        .map(|t| t.0);
                }
            },
            l if l.starts_with(":") => {
                if let Some(&mut (_, ref mut pb, _)) = managed_id.as_ref().and_then(|id| p.playing.get_mut(id)) {
                    if let Ok(t) = l[1..].parse() {
                        pb.set_position_time(time::Duration::new(t, 0));
                    }
                }
            },
            l if l.starts_with("t") => {
                if let Some(&mut (_, ref mut pb, _)) = managed_id.as_ref().and_then(|id| p.playing.get_mut(id)) {
                    if let Ok(r) = l[1..].parse() {
                        pb.set_tempo(r);
                    }
                }
            },
            ukn => println!("wtf: {}", ukn),
        }
    }
}

fn format_duraton(dur: &time::Duration) -> String {
    let secs = dur.as_secs();
    let nanos = dur.subsec_nanos();
    format!("{:02}:{:02}.{}", secs / 60, secs % 60, nanos / 1_000_000_000)
}
