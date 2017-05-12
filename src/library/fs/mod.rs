use std::*;
use std::borrow::Cow;
use std::hash::{Hash, Hasher};
use std::sync::mpsc;
use notify::{self, Watcher};
use rusqlite as sqlite;
use xdg;
use ::format;
use ::library::{self, Library, Track, TrackInfo};

mod playlist;
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
        let database_schema = include_str!("database.sql");

        let wanted_version = {
            let mut s = collections::hash_map::DefaultHasher::new();
            database_schema.hash(&mut s);
            (s.finish() % u32::MAX as u64) as i64
        };
        let db_version = db.prepare("PRAGMA user_version")?
            .query_row(&[], |row| row.get::<_, i64>(0))?;

        debug!("db schema versions: current={}, wanted={}", db_version, wanted_version);
        let mut db = if db_version != wanted_version {
            drop(db);
            debug!("(re)initializing filesystem db");
            fs::remove_file(&db_path)?;
            let db = sqlite::Connection::open(&db_path)?;
            db.execute_batch(database_schema)?;
            // Eh, pragma statements don't seem to handle parameters very well. Let's use the
            // idiot way for now.
            db.execute(&format!("PRAGMA user_version = {}", wanted_version), &[])?;
            db
        } else {
            debug!("db schema up to date");
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
                if let Err(err) = add_to_index(&mut db, &root) {
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
                                    add_to_index(db, &path)
                                })
                        });
                    },
                    notify::DebouncedEvent::Write(path) => {
                        with_db!(|db: &mut sqlite::Connection| {
                            path.canonicalize()
                                .map_err(|err| err.into())
                                .and_then(|path| {
                                    add_to_index(db, &path)
                                })
                        });
                    },
                    notify::DebouncedEvent::Chmod(path) => {
                        with_db!(|db: &mut sqlite::Connection| {
                            path.canonicalize()
                                .map_err(|err| err.into())
                                .and_then(|path| {
                                    add_to_index(db, &path)?;
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
                                .and_then(|path| add_to_index(db, &path))
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

    /// TODO: This function will be removed in the future when the search API is finished.
    pub fn track_by_path(&self, path: &path::Path) -> Result<Option<sync::Arc<Track>>, Box<error::Error>> {
        let path = if path.is_absolute() {
            path.canonicalize()?
        } else {
            self.root.join(path).canonicalize()?
        };
        let id = path.to_str()
            .ok_or_else(|| Error::BadPath(path.to_path_buf()))?;
        let track = self.tracks()?
            .find(|track| track.id().1 == id);
        Ok(track)
    }
}

impl library::Library for Filesystem {
    fn name(&self) -> Cow<str> {
        Cow::Borrowed("fs")
    }

    fn find_by_id(&self, id: &library::Identity) -> Result<Option<library::Audio>, Box<error::Error>> {
        let (lib, id) = id.id();
        if lib == self.name() {
            unimplemented!();
        }
        let track = self.track_by_path(path::Path::new(id.as_ref()))?;
        Ok(track.map(library::Audio::Track))
    }

    fn tracks(&self) -> Result<Box<iter::Iterator<Item=sync::Arc<library::Track>>>, Box<error::Error>> {
        let db = self.db.lock().unwrap();
        let mut stmt_tracks = db.prepare(r#"
           SELECT * FROM "track"
        "#)?;
        let mut stmt_artists = db.prepare(r#"
           SELECT "name", "type" FROM "track_artist"
           WHERE "track_path" = ?1
        "#)?;
        let mut stmt_genres = db.prepare(r#"
           SELECT "genre" FROM "track_genre"
           WHERE "track_path" = ?1
        "#)?;
        let tracks: Result<Vec<_>, Box<error::Error>> = stmt_tracks
            .query_and_then(&[], |row| {
                let mut track = RawTrack {
                    path: row.get("path"),
                    modified_at: time::UNIX_EPOCH
                        + time::Duration::from_secs(row.get::<_, i64>("modified_at") as _),
                    duration: time::Duration::from_secs(row.get::<_, i64>("duration") as _),
                    title: row.get("title"),
                    artists: vec![],
                    remixers: vec![],
                    genres: vec![],
                    album_title: row.get("album_title"),
                    album_artists: vec![],
                    album_disc: row.get("album_disc"),
                    album_track: row.get("album_track"),
                    rating: row.get("rating"),
                    release: row.get("release"),
                };
                let artists = stmt_artists.query_map(&[&track.path], |row| (row.get("name"), row.get("type")))?;
                for artist in artists {
                    let (name, typ): (_, Option<String>) = artist?;
                    match typ.as_ref().map(|s| s.as_str()) {
                        None => track.artists.push(name),
                        Some("album") => track.album_artists.push(name),
                        Some("remixer") => track.remixers.push(name),
                        _ => unreachable!(),
                    };
                }
                for genre in stmt_genres.query_map(&[&track.path], |row| row.get("genre"))? {
                    track.genres.push(genre?);
                }
                Ok(track)
            })?
            .collect(); // TODO: Stream results instead of collecting.
        Ok(Box::from(tracks?.into_iter().map(|t| sync::Arc::<library::Track>::from(sync::Arc::from(t)))))
    }
}

/// Creates an ad-hoc track from a path.
pub fn track_from_path(path: &path::Path) -> Result<sync::Arc<Track>, Error> {
    let (_, metadata) = format::decode_file(path)?;
    Ok(sync::Arc::new(MetadataTrack {
        path: path.to_path_buf(),
        meta: metadata,
    }))
}


/// Attempts to recursively add or update a file or directory to the index.
fn add_to_index(db: &mut sqlite::Connection, path: &path::Path) -> Result<(), Error> {
    fn dir_add_recursive(db: &mut sqlite::Connection, path: &path::Path) -> Result<(), Error> {
        debug_assert!(fs::metadata(path)?.is_dir());
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let ftype = entry.file_type()?;

            if ftype.is_dir() {
                dir_add_recursive(db, &*entry.path())?;
            } else if ftype.is_symlink() {
                track_add(db, &*entry.path(), &fs::metadata(entry.path())?)?;
            } else {
                track_add(db, &*entry.path(), &entry.metadata()?)?;
            }
        }
        return Ok(());
    }
    fn track_add(db: &mut sqlite::Connection, path: &path::Path, metadata: &fs::Metadata) -> Result<(), Error> {
        assert!(metadata.is_file());

        // Check whether the index is outdated by comparing the timestamp of the file with the one
        // in the database.
        let mtime = Timestamp(metadata.modified()?);
        let path_str = path.to_str()
            .ok_or(Error::BadPath(path.to_path_buf()))?;
        let up_to_date = db.query_row(r#"
            SELECT COUNT(*) AS "num" FROM "track"
            WHERE "path" = ?1
            AND "modified_at" = ?2
            LIMIT 1
        "#, &[
            &path_str, &mtime], |row| row.get::<_, bool>("num"))?;
        if up_to_date {
            debug!("skipping (up to date): {}", path.to_string_lossy());
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

    let metadata = fs::metadata(path)?;
    if metadata.file_type().is_dir() {
        dir_add_recursive(db, path)
    } else if metadata.file_type().is_symlink() {
        track_add(db, path, &fs::metadata(path)?)
    } else {
        track_add(db, path, &metadata)
    }
}

fn track_upsert<P>(db: &mut sqlite::Connection, track: &MetadataTrack<P>) -> Result<(), Error>
    where P: AsRef<path::Path> + Send + Sync {
    let tx = db.transaction()?;
    let path = track.path.as_ref().to_str()
        .ok_or(Error::BadPath(track.path.as_ref().to_path_buf()))?;
    tx.execute(r#"
        INSERT INTO "track"
        ("path", "modified_at", "duration", "title", "rating", "release", "album_title", "album_disc", "album_track")
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
    "#, &[
        &path,
        &track.modified_at().map(Timestamp),
        &(track.duration().as_secs() as i64),
        &track.title()
            .as_ref(),
        &track.rating(),
        &track.release(),
        &track.album_title()
            .as_ref()
            .map(|s| s.as_ref()),
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


struct Timestamp(time::SystemTime);

impl sqlite::types::ToSql for Timestamp {
    fn to_sql(&self) -> Result<sqlite::types::ToSqlOutput, sqlite::Error> {
        let secs = self.0.duration_since(time::UNIX_EPOCH)
            .map_err(|err| sqlite::Error::UserFunctionError(Box::from(err)))?
            .as_secs();
        Ok(sqlite::types::ToSqlOutput::Owned(sqlite::types::Value::Integer(secs as i64)))
    }
}


#[derive(Debug)]
pub enum Error {
    Format(format::Error),
    IO(io::Error),
    Sqlite(sqlite::Error),
    Xdg(xdg::BaseDirectoriesError),
    BadPath(path::PathBuf),
    NonSeek,
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
            Error::NonSeek => {
                write!(f, "Attempted to open a track that does not support seeking")
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
    use std::sync::{Arc, Mutex};
    use library::{Library, Playlist};

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
    fn build_index() {
        let fs = Filesystem::with_db(db(), path::Path::new(ALBUM)).unwrap();
        thread::sleep(time::Duration::from_secs(1)); // Await initial scan.
        let db = fs.db.lock().unwrap();
        let num_tracks = db.query_row("SELECT COUNT(*) FROM \"track\"", &[], |row| row.get(0)).unwrap();
        assert_eq!(3, num_tracks);
    }

    #[test]
    fn track_index_conversion() {
        let fs = Filesystem::with_db(db(), path::Path::new(ALBUM)).unwrap();
        thread::sleep(time::Duration::from_secs(1)); // Await initial scan.
        let file = "01 - The B-Trees - Lucy in the Cloud with Sine Waves.flac";
        let pt = track_from_path(&path::Path::new("testdata/Various Artists - Dark Sine of the Moon").join(file)).unwrap();
        let db = fs.track_by_path(path::Path::new(file)).unwrap().unwrap();
        assert_eq!(pt.title(), db.title());
        assert_eq!(pt.artists(), db.artists());
        assert_eq!(pt.remixers(), db.remixers());
        assert_eq!(pt.genres(), db.genres());
        assert_eq!(pt.album_title(), db.album_title());
        assert_eq!(pt.album_artists(), db.album_artists());
        assert_eq!(pt.album_disc(), db.album_disc());
        assert_eq!(pt.album_track(), db.album_track());
        assert_eq!(pt.rating(), db.rating());
        assert_eq!(pt.release(), db.release());
        let pt_mod = pt.modified_at().unwrap()
            .duration_since(time::UNIX_EPOCH).unwrap();
        let db_mod = db.modified_at().unwrap()
            .duration_since(time::UNIX_EPOCH).unwrap();
        assert_eq!(pt_mod.as_secs(), db_mod.as_secs());
        assert_eq!(pt.duration().as_secs(), db.duration().as_secs());
    }

    #[test]
    fn clean_tracks() {
        let db = db();
        db.execute(r#"
            INSERT INTO "track"
            ("path", "modified_at", "duration", "title")
            VALUES ('/home/user/non_existing.file', 1337, 42, 'Dummy')
        "#, &[]).unwrap();
        assert_eq!(1, db.query_row("SELECT COUNT(*) FROM \"track\"", &[], |row| row.get(0)).unwrap());

        track_clean_recursive(&db, path::Path::new("/home/user/")).unwrap();
        assert_eq!(0, db.query_row("SELECT COUNT(*) FROM \"track\"", &[], |row| row.get(0)).unwrap());
    }

    #[test]
    fn query_tracks() {
        let fs = Filesystem::with_db(db(), path::Path::new(ALBUM)).unwrap();
        thread::sleep(time::Duration::from_secs(1)); // Await initial scan.
        assert_eq!(3, fs.tracks().unwrap().count());
    }

    #[test]
    fn playlist_read() {
        let fs = Filesystem::with_db(db(), path::Path::new(ALBUM)).unwrap();
        thread::sleep(time::Duration::from_secs(1)); // Await initial scan.
        let fs = Arc::new(Mutex::new(fs));
        let playlist = playlist::Playlist {
            file: "testdata/Various Artists - Dark Sine of the Moon/00 - playlist.m3u".to_string(),
            fs: Arc::downgrade(&fs),
        };
        assert_eq!(3, playlist.len().unwrap());
        assert_eq!(3, playlist.contents().unwrap().len());
    }
}
