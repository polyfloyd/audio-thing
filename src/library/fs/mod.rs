use std::*;
use std::borrow::Cow;
use std::sync::mpsc;
use notify::{self, Watcher};
use rusqlite as sqlite;
use xdg;
use ::format;
use ::library::{Track, TrackInfo};

mod track;
use self::track::*;


pub struct Filesystem {
    root: path::PathBuf,

    /// The connection to a sqlite database used for indexing.
    db: sync::Arc<sync::Mutex<sqlite::Connection>>,
}

impl Filesystem {
    pub fn new(root: &path::Path) -> Result<Filesystem, Error> {
        // TODO: Instance
        let db_path = xdg::BaseDirectories::with_prefix(env!("CARGO_PKG_NAME"))?
            .place_cache_file("filesystem_TODO.db")?;
        let db = sqlite::Connection::open(&db_path)?;

        let current_version = env!("CARGO_PKG_VERSION_MAJOR").parse::<u32>().unwrap() * 1_00_00
            + env!("CARGO_PKG_VERSION_MINOR").parse::<u32>().unwrap() * 1_00
            + env!("CARGO_PKG_VERSION_PATCH").parse::<u32>().unwrap();
        let db_version = db.prepare("PRAGMA user_version")?
            .query_row(&[], |row| row.get::<_, u32>(0))?;

        debug!("filesystem db schema version: {}, current: {}", db_version, current_version);
        let mut db = if cfg!(not(release)) || db_version != current_version {
            drop(db);
            debug!("(re)initializing filesystem db");
            fs::remove_file(&db_path)?;
            let db = sqlite::Connection::open(&db_path)?;
            db.execute_batch(include_str!("database.sql"))?;
            // Eh, pragma statements don't seem to handle parameters very well. Let's use the
            // idiot way for now.
            db.execute(&format!("PRAGMA user_version = {}", current_version), &[])?;
            db
        } else {
            debug!("filesystem db schema up to date");
            db
        };
        init_db_functions(&mut db)?;
        Filesystem::with_db(db, root)
    }

    fn with_db(db: sqlite::Connection, root: &path::Path) -> Result<Filesystem, Error> {
        let root = root.canonicalize()?;
        assert!(root.is_absolute());
        debug!("Initializing filesystem with root: {}", root.to_string_lossy());
        let fs = Filesystem {
            root: root,
            db: sync::Arc::new(sync::Mutex::new(db)),
        };

        let root = fs.root.clone();
        let db_weak = sync::Arc::downgrade(&fs.db);
        thread::spawn(move || {
            {
                let arc = match db_weak.upgrade() {
                    Some(arc) => arc,
                    None => return,
                };
                let mut db = arc.lock().unwrap();
                if let Err(err) = track_add_recursive(&mut db, &root) {
                    error!("error building index: {}", err);
                }
                if let Err(err) = track_clean_recursive(&mut db, path::Path::new("")) {
                    error!("error cleaning index: {}", err);
                }
            }

            let (tx, rx) = mpsc::channel();
            let mut watcher: notify::RecommendedWatcher = notify::Watcher::new(tx, time::Duration::from_secs(30)).unwrap();
            watcher.watch(root, notify::RecursiveMode::Recursive).unwrap();

            for event in rx.into_iter() {
                macro_rules! with_db {
                    ($expr:expr) => {{
                        let arc = match db_weak.upgrade() {
                            Some(arc) => arc,
                            None => return,
                        };
                        let mut db = arc.lock().unwrap();
                        match $expr(&mut *db) {
                            Ok(_) => (),
                            Err(err) => {
                                error!("{}", err);
                            },
                        }
                    }}
                }
                match event {
                    notify::DebouncedEvent::Create(path) => {
                        with_db!(|db: &mut sqlite::Connection| {
                            path.canonicalize()
                                .map_err(|err| err.into())
                                .and_then(|path| {
                                    track_add_recursive(db, &path)
                                })
                        });
                    },
                    notify::DebouncedEvent::Write(path) => {
                        with_db!(|db: &mut sqlite::Connection| {
                            path.canonicalize()
                                .map_err(|err| err.into())
                                .and_then(|path| {
                                    track_add_recursive(db, &path)
                                })
                        });
                    },
                    notify::DebouncedEvent::Chmod(path) => {
                        with_db!(|db: &mut sqlite::Connection| {
                            path.canonicalize()
                                .map_err(|err| err.into())
                                .and_then(|path| {
                                    track_add_recursive(db, &path)?;
                                    track_clean_recursive(db, &path)
                                })
                        });
                    },
                    notify::DebouncedEvent::Remove(path) => {
                        with_db!(|db: &mut sqlite::Connection| {
                            path.canonicalize()
                                .map_err(|err| err.into())
                                .and_then(|path| {
                                    track_clean_recursive(db, &path)
                                })
                        });
                    },
                    notify::DebouncedEvent::Rename(src, dest) => {
                        with_db!(|db: &mut sqlite::Connection| {
                            src.canonicalize()
                                .map_err(|err| err.into())
                                .and_then(|path| track_clean_recursive(db, &path))?;
                            dest.canonicalize()
                                .map_err(|err| err.into())
                                .and_then(|path| track_add_recursive(db, &path))
                        });
                    },
                    notify::DebouncedEvent::NoticeWrite(_) => (),
                    notify::DebouncedEvent::NoticeRemove(_) => (),
                    notify::DebouncedEvent::Rescan => (),
                    notify::DebouncedEvent::Error(_, _) => (),
                }
            }

        });

        Ok(fs)
    }
}


/// Attempts to recursively add or update a file to the index.
fn track_add_recursive(db: &mut sqlite::Connection, path: &path::Path) -> Result<(), Error> {
    if fs::metadata(path)?.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            track_add_recursive(db, &*entry.path())?;
        }
        return Ok(());
    }

    let rs = format::decode_file(path);
    if let Err(format::Error::Unsupported) = rs {
        // Not an audio file.
        debug!("skipping (unsupported): {}", path.to_string_lossy());
        return Ok(());
    }
    let (_, metadata) = rs?;
    if metadata.num_samples.is_none() {
        // A stream or a track without a known length.
        debug!("skipping (unknown length): {}", path.to_string_lossy());
        return Ok(());
    }

    let track = MetadataTrack {
        path: path,
        meta: metadata,
    };
    track_upsert(db, &track)?;

    debug!("indexed {}", path.to_string_lossy());
    Ok(())
}

fn track_upsert<'a>(db: &mut sqlite::Connection, track: &MetadataTrack<'a>) -> Result<(), Error> {
    let tx = db.transaction()?;
    let path = track.path.to_str()
        .ok_or(Error::BadPath(track.path.to_path_buf()))?;
    tx.execute(r#"
        INSERT INTO "track"
        ("path", "mod_time", "duration", "title", "rating", "release", "album_title", "album_disc", "album_track")
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
    "#, &[
        &path,
        &(track.modified_at()
            .and_then(|mtime| {
                mtime.duration_since(time::UNIX_EPOCH).ok()
            })
            .map(|dur| dur.as_secs() as i64)
            .ok_or(Error::Unspecified))?,
        &(track.duration().as_secs() as i64),
        &track.title().to_string(),
        &track.rating(),
        &track.release(),
        &track.album_title().map(|s| s.to_string()),
        &track.album_disc(),
        &track.album_track(),
    ])?;
    tx.execute(r#"
        DELETE FROM "track_artist"
        WHERE "track_path" = ?1;
    "#, &[ &path ])?;

    let ar = track.artists();
    let rx = track.remixers();
    let aa = track.album_artists();
    let artists = ar.iter().map(|name| (name, None))
        .chain(rx.iter().map(|name| (name, Some("remixer"))))
        .chain(aa.iter().map(|name| (name, Some("album"))));
    for (name, typ) in artists {
        tx.execute(r#"
            INSERT INTO "track_artist"
            ("track_path", "name", "type")
            VALUES (?1, ?2, ?3)
        "#, &[ &path, name, &typ ])?;
    }
    for genre in track.genres().iter() {
        tx.execute(r#"
            INSERT INTO "track_genre"
            ("track_path", "genre")
            VALUES (?1, ?2)
        "#, &[ &path, genre ])?;
    }
    tx.commit()?;
    Ok(())
}

fn track_clean_recursive(db: &sqlite::Connection, path: &path::Path) -> Result<(), Error> {
    let path_str = path.to_str()
        .ok_or(Error::BadPath(path.to_path_buf()))?
        .to_string();
    db.execute(r#"
        DELETE FROM "track"
        WHERE "path" LIKE ?1 AND NOT file_exists("path")
    "#, &[ &(path_str + "%") ])?;
    Ok(())
}


fn init_db_functions(db: &mut sqlite::Connection) -> Result<(), Error> {
    db.create_scalar_function("file_exists", 1, false, |ctx| {
        let path = ctx.get::<String>(0)?;
        fs::metadata(path)
            .map(|meta| meta.is_file())
            .or_else(|err| {
                match err.kind() {
                    io::ErrorKind::NotFound|io::ErrorKind::PermissionDenied => Ok(false),
                    _ => Err(sqlite::Error::UserFunctionError(Box::new(err)))
                }
            })
    })?;
    Ok(())
}


#[derive(Debug)]
pub enum Error {
    Format(format::Error),
    IO(io::Error),
    Sqlite(sqlite::Error),
    Xdg(xdg::BaseDirectoriesError),
    BadPath(path::PathBuf),
    Unspecified,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::Format(ref err) =>  {
                write!(f, "Format: {}", err)
            },
            Error::IO(ref err) =>  {
                write!(f, "IO: {}", err)
            },
            Error::Sqlite(ref err) =>  {
                write!(f, "Sqlite: {}", err)
            },
            Error::Xdg(ref err) => {
                write!(f, "Xdg: {}", err)
            },
            Error::BadPath(ref path) => {
                write!(f, "The path {} could not be converted to a string", path.to_string_lossy())
            },
            Error::Unspecified => {
                write!(f, "Unspecified")
            },
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        "Filesystem library error"
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            Error::Format(ref err) => Some(err),
            Error::IO(ref err) => Some(err),
            Error::Sqlite(ref err) => Some(err),
            Error::Xdg(ref err) => Some(err),
            _ => None,
        }
    }
}

impl From<format::Error> for Error {
    fn from(err: format::Error) -> Error {
        Error::Format(err)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::IO(err)
    }
}

impl From<sqlite::Error> for Error {
    fn from(err: sqlite::Error) -> Error {
        Error::Sqlite(err)
    }
}

impl From<xdg::BaseDirectoriesError> for Error {
    fn from(err: xdg::BaseDirectoriesError) -> Error { Error::Xdg(err) }
}


#[cfg(test)]
mod tests {
    use super::*;

    const ALBUM: &'static str = "testdata/Various Artists - Dark Sine of the Moon";

    fn db() -> sqlite::Connection {
        let mut db = sqlite::Connection::open_in_memory().unwrap();
        init_db_functions(&mut db).unwrap();
        db.execute_batch(include_str!("database.sql")).unwrap();
        db
    }

    #[test]
    fn db_schema() {
        let db = sqlite::Connection::open_in_memory().unwrap();
        db.execute_batch(include_str!("database.sql")).unwrap();
    }

    #[test]
    fn db_user_funcs() {
        let mut db = sqlite::Connection::open_in_memory().unwrap();
        init_db_functions(&mut db).unwrap();
        let file = "testdata/Various Artists - Dark Sine of the Moon/01 - The B-Trees - Lucy in the Cloud with Sine Waves.flac";
        let exists = db.query_row("SELECT file_exists(?1)", &[&file], |row| row.get::<_, bool>(0)).unwrap();
        assert!(exists);
        let non_existing = db.query_row("SELECT file_exists('non_existing.file')", &[], |row| row.get::<_, bool>(0)).unwrap();
        assert!(!non_existing);
        let not_a_file = db.query_row("SELECT file_exists('testdata')", &[], |row| row.get::<_, bool>(0)).unwrap();
        assert!(!not_a_file);
    }

    #[test]
    fn read_tracks() {
        let fs = Filesystem::with_db(db(), path::Path::new(ALBUM)).unwrap();
        thread::sleep(time::Duration::from_secs(1)); // Await initial scan.
        let db = fs.db.lock().unwrap();
        let num_tracks = db.query_row("SELECT COUNT(*) FROM \"track\"", &[], |row| row.get(0)).unwrap();
        assert_eq!(3, num_tracks);
    }

    #[test]
    fn search() {
        let fs = Filesystem::with_db(db(), path::Path::new(ALBUM)).unwrap();
        thread::sleep(time::Duration::from_secs(1)); // Await initial scan.
        let db = fs.db.lock().unwrap();
        let num_tracks = db.query_row("SELECT COUNT(*) FROM \"track\"", &[], |row| row.get(0)).unwrap();
        assert_eq!(3, num_tracks);
    }

    #[test]
    fn clean_tracks() {
        let db = db();
        db.execute(r#"
            INSERT INTO "track"
            ("path", "mod_time", "duration", "title")
            VALUES ('/home/user/non_existing.file', 1337, 42, 'Dummy')
        "#, &[]).unwrap();
        assert_eq!(1, db.query_row("SELECT COUNT(*) FROM \"track\"", &[], |row| row.get(0)).unwrap());

        track_clean_recursive(&db, path::Path::new("/home/user/")).unwrap();
        assert_eq!(0, db.query_row("SELECT COUNT(*) FROM \"track\"", &[], |row| row.get(0)).unwrap());
    }
}
