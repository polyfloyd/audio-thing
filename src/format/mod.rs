use std::collections::HashMap;

pub mod flac;

#[derive(Debug, Clone)]
pub struct Metadata {
    pub sample_rate: u32,
    pub num_samples: Option<u64>,

    /// There seems to be no real standard for music tags, so all other tags read by decoders
    /// should thrown in this hashmap.
    /// Decoders should restrict keys to lowercase alphanumeric characters.
    pub tags: HashMap<String, String>,
}

impl Metadata {
    fn new() -> Metadata {
        Metadata {
            sample_rate: 0,
            num_samples: None,
            tags: HashMap::new(),
        }
    }
}
