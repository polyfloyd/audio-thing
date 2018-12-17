use crate::library::fs::*;
use crate::library::{self, Identity};
use std::borrow::Cow;
use std::io::BufRead;
use std::io::Write;
use std::sync::{Mutex, Weak};
use std::*;

/// A playlist that can loads and stores itself to M3U.
///
/// The standard format for an M3U playlist defines a `#EXTM3U` header followed by mutliple entries
/// consisting of an `#EXTINF` line containing the length and title of the track, and the path to
/// the entry:
/// ```m3u
/// #EXTM3U
/// #EXTINF:30,The B-Trees - Lucy in the Cloud with Sine Waves
/// 01 - The B-Trees - Lucy in the Cloud with Sine Waves.flac
/// #EXTINF:30,Michael FLACson - One or Zero
/// 02 - Michael FLACson - One or Zero.flac
/// #EXTINF:30,DJ Testo Ft. Curry RAII Yepsend - call() me Maybe<T>
/// 03 - DJ Testo Ft. Curry RAII Yepsend - call() me Maybe<T>.flac
/// ```
pub struct Playlist {
    pub file: String,
    pub fs: Weak<Mutex<Filesystem>>,
}

impl library::Identity for Playlist {
    fn id(&self) -> (Cow<str>, Cow<str>) {
        (Cow::Borrowed("fs"), Cow::Borrowed(&self.file))
    }
}

impl library::Playlist for Playlist {
    fn len(&self) -> Result<usize, Box<error::Error>> {
        let mut count = 0;
        for line in io::BufReader::new(fs::File::open(&self.file)?).lines() {
            let line = line?;
            if !line.is_empty() && !line.starts_with('#') {
                count += 1;
            }
        }
        Ok(count)
    }

    fn contents(&self) -> Result<Cow<[library::Audio]>, Box<error::Error>> {
        let fs = self.fs.upgrade().ok_or_else(|| Error::Unspecified)?;
        let contents = read_m3u(&*fs.lock().unwrap(), path::Path::new(&self.file))?;
        Ok(Cow::Owned(contents))
    }

    fn as_mut(&mut self) -> Option<&mut library::PlaylistMut> {
        Some(self)
    }
}

impl library::PlaylistMut for Playlist {
    fn set_contents(&mut self, contents: &[&library::Identity]) -> Result<(), Box<error::Error>> {
        let fs_arc = self.fs.upgrade().ok_or(Error::Unspecified)?;
        let fs = fs_arc.lock().unwrap();
        let mut file = fs::File::open(&self.file)?;
        writeln!(file, "#EXTM3U")?;
        for entry in contents {
            let (lib, id) = entry.id();
            if lib != self.id().0 {
                return Err(Box::from(Error::Unspecified));
            }
            if let Some(track) = fs.track_by_path(path::Path::new(&*id))? {
                writeln!(
                    file,
                    "#EXTINF:{},{}",
                    track.duration().as_secs(),
                    track.title()
                )?;
            } else {
                writeln!(file, "#EXTINF:0,")?;
            }
            writeln!(file, "{}", id)?;
        }
        file.flush()?;
        Ok(())
    }
}

fn read_m3u(fs: &Filesystem, file: &path::Path) -> Result<Vec<library::Audio>, Box<error::Error>> {
    let mut contents = Vec::new();
    for line in io::BufReader::new(fs::File::open(file)?).lines() {
        let line = line?;
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let entry = path::Path::new(&line);
        let path = if entry.is_absolute() {
            Cow::Borrowed(entry)
        } else {
            let path = path::Path::new(file)
                .parent()
                .ok_or(Error::Unspecified)?
                .join(entry)
                .canonicalize()?;
            Cow::Owned(path)
        };
        let track = fs.track_by_path(&path)?.map(|t| library::Audio::Track(t));
        contents.extend(track.into_iter());
    }
    Ok(contents)
}
