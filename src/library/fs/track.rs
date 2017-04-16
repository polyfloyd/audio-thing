use std::*;
use std::borrow::Cow;
use regex::Regex;
use ::audio::*;
use ::format;
use ::library;


pub struct MetadataTrack<'a> {
    pub path: &'a path::Path,
    pub meta: format::Metadata,
}

impl<'a> library::Identity for MetadataTrack<'a> {
    fn id(&self) -> (Cow<str>, Cow<str>) {
        ("fs".into(), self.path.to_string_lossy().into())
    }
}

impl<'a> library::TrackInfo for MetadataTrack<'a> {
    fn title(&self) -> String {
        lazy_static! {
            static ref FROM_STEM: Regex = Regex::new(r"^(?:.* - .*)* - (.+)$").unwrap();
        }
        self.meta.tags.get("title")
            .map(|t| t.clone())
            .unwrap_or_else(|| {
                let stem = self.path.file_stem()
                    .unwrap()
                    .to_string_lossy();
                FROM_STEM.captures(&*stem)
                    .and_then(|cap| cap.get(1))
                    .map(|m| m.as_str().into())
                    .unwrap_or_else(|| stem.into())
            })
    }

    fn artists(&self) -> Vec<String> {
        lazy_static! {
            static ref FROM_STEM: Regex = Regex::new(r"^(?:.* - )(.+) - (:?.+)$").unwrap();
        }
        self.meta.tags.get("artist")
            .map(|a| vec![a.clone()])
            .unwrap_or_else(|| {
                let stem = self.path.file_stem()
                    .unwrap()
                    .to_string_lossy();
                FROM_STEM.captures(&*stem)
                    .and_then(|cap| cap.get(1))
                    .map(|m| vec![m.as_str().into()])
                    .unwrap_or(vec![])
            })
    }

    fn remixers(&self) -> Vec<String> {
        vec![]
    }

    fn genres(&self) -> Vec<String> {
        self.meta.tags.get("genre")
            .map(|g| {
                g.split(',')
                    .map(|t| t.trim().to_string())
                    .collect()
            })
            .unwrap_or(vec![])
    }

    fn album_title(&self) -> Option<String> {
        self.meta.tags.get("album")
            .map(|t| t.clone())
    }

    fn album_artists(&self) -> Vec<String> {
        self.meta.tags.get("albumartist")
            .map(|a| vec![a.clone()])
            .unwrap_or(vec![])
    }

    fn album_disc(&self) -> Option<i32> {
        self.meta.tags.get("discnumber")
            .and_then(|t| t.parse().ok())
    }

    fn album_track(&self) -> Option<i32> {
        lazy_static! {
            static ref FROM_STEM: Regex = Regex::new(r"^0*([1-9]\d*)").unwrap();
        }
        self.meta.tags.get("tracknumber")
            .and_then(|t| t.parse().ok())
            .or_else(|| {
                let stem = self.path.file_stem()
                    .unwrap()
                    .to_string_lossy();
                FROM_STEM.captures(&*stem)
                    .and_then(|cap| cap.get(1))
                    .and_then(|m| m.as_str().parse().ok())
            })
    }

    fn rating(&self) -> Option<u8> {
        None
    }

    fn release(&self) -> Option<library::Release> {
        self.meta.tags.get("date").into_iter()
            .chain(self.meta.tags.get("retaildate").into_iter())
            .filter_map(|t| t.parse().ok())
            .fold(None, |acc: Option<library::Release>, b| {
                match acc {
                    None => Some(b),
                    Some(acc) => Some(acc.most_precise(b)),
                }
            })
    }
}

impl<'a> library::Track for MetadataTrack<'a> {
    fn modified_at(&self) -> Option<time::SystemTime> {
        fs::metadata(self.path)
            .and_then(|stat| stat.modified())
            .ok()
    }

    fn audio(&self) -> Result<dyn::Seek, Box<error::Error>> {
        unimplemented!();
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
    use std::collections::HashMap;
    use ::library::TrackInfo;

    #[test]
    fn test_tags() {
        let track = MetadataTrack {
            path: path::Path::new("/home/user/Music/VA - Unknown.flac"),
            meta: format::Metadata {
                sample_rate: 44100,
                num_samples: Some(1_000_000),
                tags: [
                    ("title", "Sandstorm"),
                    ("artist", "Darude"),
                    ("genre", "Trance"),
                ].into_iter().map(|&(k, v)| (k.into(), v.into())).collect(),
            },
        };
        assert_eq!("Sandstorm", track.title());
        assert_eq!(vec!["Darude"], track.artists());
        assert_eq!(vec!["Trance"], track.genres());
    }

    #[test]
    fn test_tags_from_filename() {
        let track = MetadataTrack {
            path: path::Path::new("/home/user/Music/01 - Darude - Sandstorm.flac"),
            meta: format::Metadata {
                sample_rate: 44100,
                num_samples: Some(1_000_000),
                tags: HashMap::new(),
            },
        };
        assert_eq!("Sandstorm", track.title());
        assert_eq!(vec!["Darude"], track.artists());
        assert_eq!(Some(1), track.album_track());
    }
}
