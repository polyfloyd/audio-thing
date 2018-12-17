#![cfg_attr(all(test, feature = "unstable"), feature(test))]
#![allow(unused)]

#[macro_use]
extern crate derive_error;

use crate::library::Library;
use std::io::{BufRead, Write};
use std::*;

mod audio;
mod filter;
mod format;
mod library;
mod player;
mod pulse;

fn main() {
    if cfg!(release) || env::var("LOG").is_ok() {
        badlog::init_from_env("LOG");
    } else {
        badlog::init(Some("debug"));
    }

    let fs = sync::Arc::new(library::fs::Filesystem::new(path::Path::new("testdata")).unwrap());
    let libs: Vec<sync::Arc<library::Library>> = vec![fs.clone()];
    let player = player::Player::new(Box::new(player::output::pulse::Output {}), libs);

    let mut managed_id = env::args().nth(1).map(|filename| {
        let mut p = player.lock().unwrap();
        let path = path::PathBuf::from(filename);
        let track = library::fs::track_from_path(&path).unwrap();
        p.queue.push(library::Audio::Track(track));
        let (id, _) = p.play_next_from_queue().unwrap().unwrap();
        id
    });

    let mut out = io::stderr();
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();
    while let Some(Ok(line)) = lines.next() {
        let mut p = player.lock().unwrap();
        match line.as_ref() {
            "list" => {
                let tracks: Vec<_> = fs.tracks().unwrap().collect();
                for (i, track) in tracks.iter().enumerate() {
                    writeln!(
                        out,
                        "{}: {} - {}",
                        i,
                        track.artists().get(0).unwrap_or(&"?".to_string()),
                        track.title()
                    )
                    .unwrap();
                }
                let track = lines
                    .next()
                    .unwrap()
                    .unwrap_or_else(|_| "q".to_string())
                    .parse()
                    .ok()
                    .and_then(|index| tracks.into_iter().nth(index));
                if let Some(track) = track {
                    p.queue.push(library::Audio::Track(track));
                    if let Some(&mut (_, ref mut pb, _)) =
                        managed_id.as_ref().and_then(|id| p.playing.get_mut(id))
                    {
                        pb.set_state(player::State::Stopped)
                    }
                    let i = p.queue.len() - 1;
                    let (id, _) = p.play_from_queue(i).unwrap().unwrap();
                    managed_id = Some(id);
                }
            }
            "prev" => {
                managed_id = p.play_previous_from_queue().unwrap().map(|t| t.0);
            }
            "next" => {
                managed_id = p.play_next_from_queue().unwrap().map(|t| t.0);
            }

            "pause" => {
                if let Some(&mut (_, ref mut pb, _)) =
                    managed_id.as_ref().and_then(|id| p.playing.get_mut(id))
                {
                    pb.set_state(player::State::Paused);
                }
            }
            "play" => {
                if let Some(&mut (_, ref mut pb, _)) =
                    managed_id.as_ref().and_then(|id| p.playing.get_mut(id))
                {
                    pb.set_state(player::State::Playing);
                }
            }
            "stop" => {
                if let Some(&mut (_, ref mut pb, _)) =
                    managed_id.as_ref().and_then(|id| p.playing.get_mut(id))
                {
                    pb.set_state(player::State::Stopped);
                }
            }
            "info" => {
                fn print_info<T: library::TrackInfo + ?Sized>(info: &T) {
                    let mut out = io::stderr();
                    writeln!(out, "meta:").unwrap();
                    writeln!(out, "  title:   {}", info.title()).unwrap();
                    writeln!(
                        out,
                        "  artists: {}",
                        info.artists().get(0).unwrap_or(&"?".to_string())
                    )
                    .unwrap();
                    writeln!(
                        out,
                        "  rating:  {}",
                        info.rating()
                            .map(|r| format!("{}", r))
                            .unwrap_or_else(|| "-".to_string())
                    )
                    .unwrap();
                }
                if let Some(&mut (ref audio, ref mut pb, ref info)) =
                    managed_id.as_ref().and_then(|id| p.playing.get_mut(id))
                {
                    let duration = pb
                        .duration_time()
                        .map(|d| format_duraton(&d))
                        .unwrap_or_else(|| "∞".to_string());
                    info.as_ref()
                        .map(|i| print_info(i.as_ref()))
                        .or_else(|| audio.track().map(print_info));
                    writeln!(out, "state:    {:?}", pb.state()).unwrap();
                    writeln!(
                        out,
                        "position: {}/{}",
                        format_duraton(&pb.position_time()),
                        duration
                    )
                    .unwrap();
                    writeln!(out, "tempo:    {}", pb.tempo()).unwrap();
                    writeln!(
                        out,
                        "latency:  {}ns",
                        pb.stream.latency().unwrap().subsec_nanos()
                    )
                    .unwrap();
                }
            }

            "queue" => {
                for (i, audio) in p.queue.iter().enumerate() {
                    if p.queue_cursor == Some(i) {
                        write!(out, "-> ").unwrap();
                    } else {
                        write!(out, "   ").unwrap();
                    }
                    match *audio {
                        library::Audio::Track(ref track) => {
                            writeln!(
                                out,
                                "{} - {}",
                                track.artists().get(0).unwrap_or(&"?".to_string()),
                                track.title()
                            )
                            .unwrap();
                        }
                        library::Audio::Stream(_) => unimplemented!(),
                    };
                }
            }

            l if l.starts_with('j') => {
                if let Ok(i) = l[1..].parse() {
                    managed_id = p.play_from_queue(i).unwrap().map(|t| t.0);
                }
            }
            l if l.starts_with(':') => {
                if let Some(&mut (_, ref mut pb, _)) =
                    managed_id.as_ref().and_then(|id| p.playing.get_mut(id))
                {
                    if let Ok(t) = l[1..].parse() {
                        pb.set_position_time(time::Duration::new(t, 0));
                    }
                }
            }
            l if l.starts_with('t') => {
                if let Some(&mut (_, ref mut pb, _)) =
                    managed_id.as_ref().and_then(|id| p.playing.get_mut(id))
                {
                    if let Ok(r) = l[1..].parse() {
                        pb.set_tempo(r);
                    }
                }
            }
            ukn => writeln!(out, "wtf: {}", ukn).unwrap(),
        }
    }
}

fn format_duraton(dur: &time::Duration) -> String {
    let secs = dur.as_secs();
    let nanos = dur.subsec_nanos();
    format!(
        "{:02}:{:02}.{}",
        secs / 60,
        secs % 60,
        nanos / 1_000_000_000
    )
}
