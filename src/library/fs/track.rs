use std::*;
use std::borrow::Cow;
use regex::Regex;
use ::audio::*;
use ::format;
use ::library;
use super::Error;


pub struct RawTrack {
    pub path: String,
    pub modified_at: time::SystemTime,
    pub duration: time::Duration,

    pub title: String,
    pub artists: Vec<String>,
    pub remixers: Vec<String>,
    pub genres: Vec<String>,
    pub album_title: Option<String>,
    pub album_artists: Vec<String>,
    pub album_disc: Option<i32>,
    pub album_track: Option<i32>,
    pub rating: Option<u8>,
    pub release: Option<library::Release>,
}

impl library::Identity for RawTrack {
    fn id(&self) -> (Cow<str>, Cow<str>) {
        (Cow::Borrowed("fs"), Cow::Borrowed(&self.path))
    }
}

impl library::TrackInfo for RawTrack {
    fn title(&self) -> Cow<str> {
        Cow::Borrowed(&self.title)
    }

    fn artists(&self) -> Cow<[String]> {
        Cow::Borrowed(&self.artists)
    }

    fn remixers(&self) -> Cow<[String]> {
        Cow::Borrowed(&self.remixers)
    }

    fn genres(&self) -> Cow<[String]> {
        Cow::Borrowed(&self.genres)
    }

    fn album_title(&self) -> Option<Cow<str>> {
        self.album_title.as_ref()
            .map(|s| Cow::Borrowed(s.as_str()))
    }

    fn album_artists(&self) -> Cow<[String]> {
        Cow::Borrowed(&self.album_artists)
    }

    fn album_disc(&self) -> Option<i32> {
        self.album_disc
    }

    fn album_track(&self) -> Option<i32> {
        self.album_track
    }

    fn rating(&self) -> Option<u8> {
        self.rating
    }

    fn release(&self) -> Option<library::Release> {
        self.release.clone()
    }
}

impl library::Track for RawTrack {
    fn modified_at(&self) -> Option<time::SystemTime> {
        Some(self.modified_at)
    }

    fn audio(&self) -> Result<dyn::Seek, Box<error::Error>> {
        let (decoder, _) = format::decode_file(path::Path::new(&self.path))?;
        decoder.into_seek()
            .ok_or(Box::from(Error::NonSeek))
    }

    fn duration(&self) -> time::Duration {
        self.duration
    }
}


pub struct MetadataTrack<P>
    where P: AsRef<path::Path> + Send + Sync {
    pub path: P,
    pub meta: format::Metadata,
}

impl<P> library::Identity for MetadataTrack<P>
    where P: AsRef<path::Path> + Send + Sync {
    fn id(&self) -> (Cow<str>, Cow<str>) {
        ("fs".into(), self.path.as_ref().to_string_lossy())
    }
}

impl<P> library::TrackInfo for MetadataTrack<P>
    where P: AsRef<path::Path> + Send + Sync {
    fn title(&self) -> Cow<str> {
        lazy_static! {
            static ref FROM_STEM: Regex = Regex::new(r"^(?:.* - .*)* - (.+)$").unwrap();
        }
        self.meta.tag.as_ref()
            .and_then(|t| t.title())
            .map(|t| Cow::Borrowed(t))
            .unwrap_or_else(|| {
                let stem = self.path.as_ref().file_stem()
                    .unwrap()
                    .to_string_lossy();
                FROM_STEM.captures(&*stem)
                    .and_then(|cap| cap.get(1))
                    .map(|m| m.as_str().to_string().into())
                    .unwrap_or(stem)
            })
    }

    fn artists(&self) -> Cow<[String]> {
        lazy_static! {
            static ref FROM_STEM: Regex = Regex::new(r"^(?:.* - )(.+) - (:?.+)$").unwrap();
        }
        self.meta.tag.as_ref()
            .and_then(|t| t.artist())
            .map(|a| vec![a.to_string()])
            .unwrap_or_else(|| {
                let stem = self.path.as_ref().file_stem()
                    .unwrap()
                    .to_string_lossy();
                FROM_STEM.captures(&*stem)
                    .and_then(|cap| cap.get(1))
                    .map(|m| vec![m.as_str().into()])
                    .unwrap_or(vec![])
            })
            .into()
    }

    fn remixers(&self) -> Cow<[String]> {
        Cow::Borrowed(&[])
    }

    fn genres(&self) -> Cow<[String]> {
        self.meta.tag.as_ref()
            .and_then(|t| t.genre())
            .map(|g| {
                g.split(',')
                    .map(|t| t.trim().to_string())
                    .collect()
            })
            .unwrap_or(vec![])
            .into()
    }

    fn album_title(&self) -> Option<Cow<str>> {
        self.meta.tag.as_ref()
            .and_then(|t| t.album())
            .map(|t| Cow::Borrowed(t))
    }

    fn album_artists(&self) -> Cow<[String]> {
        self.meta.tag.as_ref()
            .and_then(|t| t.album_artist())
            .map(|a| vec![a.to_string()])
            .unwrap_or(vec![])
            .into()
    }

    fn album_disc(&self) -> Option<i32> {
        self.meta.tag.as_ref()
            .and_then(|t| t.disc())
            .map(|i| i as i32)
    }

    fn album_track(&self) -> Option<i32> {
        lazy_static! {
            static ref FROM_STEM: Regex = Regex::new(r"^0*([1-9]\d*)").unwrap();
        }
        self.meta.tag.as_ref()
            .and_then(|t| t.track())
            .map(|i| i as i32)
            .or_else(|| {
                let stem = self.path.as_ref().file_stem()
                    .unwrap()
                    .to_string_lossy();
                FROM_STEM.captures(&*stem)
                    .and_then(|cap| cap.get(1))
                    .and_then(|m| m.as_str().parse().ok())
            })
    }

    fn rating(&self) -> Option<u8> {
        self.meta.tag.as_ref()
            .and_then(|t| t.get("POPM"))
            .and_then(|frame| frame.content.unknown())
            .and_then(|data| {
                data.iter()
                    .position(|b| *b == 0)
                    .and_then(|i| data.get(i + 1))
            })
            .and_then(|num| match *num {
                0 => None,
                1...31 => Some(1),
                32...95 => Some(2),
                96...159 => Some(3),
                160...223 => Some(4),
                _ => Some(5),
            })
    }

    fn release(&self) -> Option<library::Release> {
        self.meta.tag.as_ref()
            .and_then(|t| {
                t.get_all("TDRL")
                    .into_iter()
                    .filter_map(|frame| frame.content.text())
                    .filter_map(|st| st.parse().ok())
                    .fold(None, |acc: Option<library::Release>, b| match acc {
                        None => Some(b),
                        Some(acc) => Some(acc.most_precise(b)),
                    })
            })
    }
}

impl<P> library::Track for MetadataTrack<P>
    where P: AsRef<path::Path> + Send + Sync {
    fn modified_at(&self) -> Option<time::SystemTime> {
        fs::metadata(&self.path)
            .and_then(|stat| stat.modified())
            .ok()
    }

    fn audio(&self) -> Result<dyn::Seek, Box<error::Error>> {
        let (decoder, _) = format::decode_file(&self.path)?;
        decoder.into_seek()
            .ok_or(Box::from(Error::NonSeek))
    }

    fn duration(&self) -> time::Duration {
        let num_samples = self.meta.num_samples
            .expect("Unkown number of samples");
        duration_of(self.meta.sample_rate, num_samples)
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use id3;
    use ::library::TrackInfo;

    #[test]
    fn test_tags() {
        let track = MetadataTrack {
            path: path::Path::new("/home/user/Music/VA - Unknown.flac"),
            meta: format::Metadata {
                sample_rate: 44100,
                num_samples: Some(1_000_000),
                tag: {
                    let mut tag = id3::Tag::new();
                    tag.set_title("Sandstorm");
                    tag.set_artist("Darude");
                    tag.set_genre("Trance");
                    Some(tag)
                },
            },
        };
        assert_eq!("Sandstorm", track.title());
        assert_eq!(vec!["Darude"], track.artists().into_owned());
        assert_eq!(vec!["Trance"], track.genres().into_owned());
    }

    #[test]
    fn test_tag_from_filename() {
        let track = MetadataTrack {
            path: path::Path::new("/home/user/Music/01 - Darude - Sandstorm.flac"),
            meta: format::Metadata {
                sample_rate: 44100,
                num_samples: Some(1_000_000),
                tag: None,
            },
        };
        assert_eq!("Sandstorm", track.title());
        assert_eq!(vec!["Darude"], track.artists().into_owned());
        assert_eq!(Some(1), track.album_track());
    }
}
