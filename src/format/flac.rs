use std::*;
use std::borrow::Cow;
use std::ops::DerefMut;
use id3;
use libflac_sys::*;
use sample::{self, I24};
use ::audio::*;
use ::format;


pub const MAGIC: &'static [u8] = b"fLaC";


struct Decoder<F, R>
    where F: sample::Frame,
          F::Sample: DecodeSample,
          R: io::Read + io::Seek + SeekExt {
    decoder: *mut FLAC__StreamDecoder,
    cb_data: Box<DecoderCallbackData<R>>,

    /// The index of the next read sample in the current block.
    current_sample: usize,
    /// The absolute position in the stream.
    abs_position: u64,
    sample_rate: u32,

    _f: marker::PhantomData<F>,
}

struct DecoderCallbackData<R> {
    input: R,
    current_block: Option<Block>,
    meta: Option<format::Metadata>,
}

pub fn decode<R>(input: R) -> Result<(dyn::Audio, format::Metadata), Error>
    where R: io::Read + io::Seek + SeekExt + 'static {
    unsafe {
        let decoder = FLAC__stream_decoder_new();
        if decoder.is_null() {
            return Err(Error::ConstructionFailed);
        }

        let mut cb_data = Box::new(DecoderCallbackData{
            input: input,
            current_block: None,
            meta: Some(format::Metadata {
                sample_rate: 0,
                num_samples: None,
                tag: Some(id3::Tag::new()),
            }),
        });

        assert!(FLAC__stream_decoder_set_metadata_respond_all(decoder) == 1);
        let init_status = FLAC__stream_decoder_init_stream(
            decoder,
            Some(read_cb::<R>),
            Some(seek_cb::<R>),
            Some(tell_cb::<R>),
            Some(length_cb::<R>),
            Some(eof_cb::<R>),
            Some(write_cb::<R>),
            Some(metadata_cb::<R>),
            Some(error_cb),
            cb_data.deref_mut() as *mut _ as _,
        );
        if init_status != FLAC__StreamDecoderInitStatus::FLAC__STREAM_DECODER_INIT_STATUS_OK {
            FLAC__stream_decoder_delete(decoder);
            return Err(Error::InitFailed(init_status));
        }

        if FLAC__stream_decoder_process_until_end_of_metadata(decoder) != 1 {
            let state = FLAC__stream_decoder_get_state(decoder);
            FLAC__stream_decoder_delete(decoder);
            return Err(Error::BadState(state));
        }
        if FLAC__stream_decoder_process_single(decoder) != 1 {
            let state = FLAC__stream_decoder_get_state(decoder);
            FLAC__stream_decoder_delete(decoder);
            return Err(Error::BadState(state));
        }

        let num_channels = FLAC__stream_decoder_get_channels(decoder);
        let sample_size = FLAC__stream_decoder_get_bits_per_sample(decoder);
        let sample_rate = FLAC__stream_decoder_get_sample_rate(decoder);
        let length = FLAC__stream_decoder_get_total_samples(decoder);
        cb_data.meta.as_mut().unwrap().sample_rate = sample_rate;
        if length > 0 {
            cb_data.meta.as_mut().unwrap().num_samples = Some(length);
            debug!("stream info: {} channels, {} bits, {} hz, {} samples", num_channels, sample_size, sample_rate, length);
        } else {
            debug!("stream info: {} channels, {} bits, {} hz, length unknown", num_channels, sample_size, sample_rate);
        }
        assert_ne!(0, cb_data.meta.as_mut().unwrap().sample_rate);

        macro_rules! dyn_type {
            ($dyn:path) => {
                $dyn(Box::from(Decoder {
                    decoder: decoder,
                    cb_data: cb_data,
                    current_sample: 0,
                    abs_position: 0,
                    sample_rate: sample_rate,
                    _f: marker::PhantomData,
                })).into()
            }
        }
        let meta = cb_data.meta.take().unwrap();
        Ok((match (length != 0, num_channels, sample_size) {
            (false, 1, 8)  => dyn_type!(dyn::Source::MonoI8),
            (false, 1, 16) => dyn_type!(dyn::Source::MonoI16),
            (false, 1, 24) => dyn_type!(dyn::Source::MonoI24),
            (false, 2, 8) => dyn_type!(dyn::Source::StereoI8),
            (false, 2, 16) => dyn_type!(dyn::Source::StereoI16),
            (false, 2, 24) => dyn_type!(dyn::Source::StereoI24),
            (true, 1, 8) => dyn_type!(dyn::Seek::MonoI8),
            (true, 1, 16) => dyn_type!(dyn::Seek::MonoI16),
            (true, 1, 24) => dyn_type!(dyn::Seek::MonoI24),
            (true, 2, 8) => dyn_type!(dyn::Seek::StereoI8),
            (true, 2, 16) => dyn_type!(dyn::Seek::StereoI16),
            (true, 2, 24) => dyn_type!(dyn::Seek::StereoI24),
            (kl, nc, ss) => return Err(Error::Unimplemented {
                known_length: kl,
                num_channels: nc,
                sample_size: ss,
            }),
        }, meta))
    }
}

impl<F, R> iter::Iterator for Decoder<F, R>
    where F: sample::Frame,
          F::Sample: DecodeSample,
          R: io::Read + io::Seek + SeekExt {
    type Item = F;
    fn next(&mut self) -> Option<Self::Item> {
        if self.cb_data.current_block.is_none() {
            self.current_sample = 0;
            unsafe {
                let error = FLAC__stream_decoder_process_single(self.decoder) != 1;
                let state = FLAC__stream_decoder_get_state(self.decoder);
                if state == FLAC__STREAM_DECODER_END_OF_STREAM || error {
                    if error {
                        error!("{}", Error::BadState(state));
                    }
                    return None;
                }
            }
            assert!(self.cb_data.current_block.is_some());
        }

        let (frame, block_consumed) = {
            let block = self.cb_data.current_block.as_ref().unwrap();
            let frame = {
                let cs = self.current_sample;
                F::from_fn(|ch| {
                    F::Sample::decode(block.data[ch][cs])
                })
            };
            self.current_sample += 1;
            (frame, self.current_sample == block.data[0].len())
        };
        if block_consumed {
            self.cb_data.current_block = None;
        }

        self.abs_position += 1;
        Some(frame)
    }
}

impl<F, R> Source for Decoder<F, R>
    where F: sample::Frame,
          F::Sample: DecodeSample,
          R: io::Read + io::Seek + SeekExt {
    fn sample_rate(&self) -> u32 { self.sample_rate }
}

impl<F, R> Seekable for Decoder<F, R>
    where F: sample::Frame,
          F::Sample: DecodeSample,
          R: io::Read + io::Seek + SeekExt {
    fn seek(&mut self, position: u64) -> Result<(), SeekError> {
        if position >= self.length() {
            return Err(SeekError::OutofRange{
                pos: position,
                size: self.length(),
            });
        }
        unsafe {
            if FLAC__stream_decoder_seek_absolute(self.decoder, position) != 1 {
                let state = FLAC__stream_decoder_get_state(self.decoder);
                return Err(SeekError::Other(Box::from(Error::BadState(state))));
            }
        }
        self.current_sample = 0;
        self.abs_position = position;
        Ok(())
    }

    fn length(&self) -> u64 {
        let total_samples = unsafe {
            FLAC__stream_decoder_get_total_samples(self.decoder)
        };
        assert_ne!(0, total_samples);
        total_samples
    }

    fn current_position(&self) -> u64 {
        self.abs_position
    }
}

impl<F, R> Seek for Decoder<F, R>
    where F: sample::Frame,
          F::Sample: DecodeSample,
          R: io::Read + io::Seek + SeekExt { }

unsafe impl<F, R> Send for Decoder<F, R>
    where F: sample::Frame,
          F::Sample: DecodeSample,
          R: io::Read + io::Seek + SeekExt { }

impl<F, R> Drop for Decoder<F, R>
    where F: sample::Frame,
          F::Sample: DecodeSample,
          R: io::Read + io::Seek + SeekExt {
    fn drop(&mut self) {
        unsafe {
            FLAC__stream_decoder_delete(self.decoder);
        }
    }
}


trait DecodeSample {
    fn decode(s: i32) -> Self;
}

impl DecodeSample for i8 {
    fn decode(s: i32) -> i8 { s as i8 }
}

impl DecodeSample for i16 {
    fn decode(s: i32) -> i16 { s as i16 }
}

impl DecodeSample for I24 {
    fn decode(s: i32) -> I24 { I24::new_unchecked(s) }
}


struct Block {
    data: Vec<Vec<i32>>,
}


unsafe extern "C" fn error_cb(_: *const FLAC__StreamDecoder, status: FLAC__StreamDecoderErrorStatus, _: *mut os::raw::c_void) {
    error!("FLAC error callback called: {:?}", status);
}

unsafe extern "C" fn write_cb<R>(_: *const FLAC__StreamDecoder, frame: *const FLAC__Frame, buffer: *const *const FLAC__int32, client_data: *mut os::raw::c_void) -> FLAC__StreamDecoderWriteStatus
    where R: io::Read + io::Seek {
    let data = (client_data as *mut DecoderCallbackData<R>).as_mut().unwrap();

    let fr = frame.as_ref().unwrap();
    let block_data = (0..fr.header.channels)
        .map(|ch| {
            let chan_base_ptr = *buffer.offset(ch as isize);
            (0..fr.header.blocksize)
                .map(|i| *chan_base_ptr.offset(i as isize))
                .collect()
        })
        .collect();
    data.current_block = Some(Block{ data: block_data });
    FLAC__StreamDecoderWriteStatus::FLAC__STREAM_DECODER_WRITE_STATUS_CONTINUE
}

unsafe extern "C" fn read_cb<R>(_: *const FLAC__StreamDecoder, buffer: *mut u8, bytes: *mut usize, client_data: *mut os::raw::c_void) -> FLAC__StreamDecoderReadStatus
    where R: io::Read {
    let mut data = (client_data as *mut DecoderCallbackData<R>).as_mut().unwrap();

    let mut buf = slice::from_raw_parts_mut(buffer, *bytes);
    *bytes = match data.input.read(&mut buf) {
        Ok(read) => read,
        Err(_) => return FLAC__StreamDecoderReadStatus::FLAC__STREAM_DECODER_READ_STATUS_ABORT,
    };
    if *bytes == 0 {
        FLAC__StreamDecoderReadStatus::FLAC__STREAM_DECODER_READ_STATUS_END_OF_STREAM
    } else {
        FLAC__StreamDecoderReadStatus::FLAC__STREAM_DECODER_READ_STATUS_CONTINUE
    }
}

unsafe extern "C" fn seek_cb<R>(_: *const FLAC__StreamDecoder, absolute_byte_offset: u64, client_data: *mut os::raw::c_void) -> FLAC__StreamDecoderSeekStatus
    where R: io::Read + io::Seek {
    let mut data = (client_data as *mut DecoderCallbackData<R>).as_mut().unwrap();
    match data.input.seek(io::SeekFrom::Start(absolute_byte_offset)) {
        Ok(_) => FLAC__StreamDecoderSeekStatus::FLAC__STREAM_DECODER_SEEK_STATUS_OK,
        Err(_) => FLAC__StreamDecoderSeekStatus::FLAC__STREAM_DECODER_SEEK_STATUS_ERROR,
    }
}

unsafe extern "C" fn tell_cb<R>(_: *const FLAC__StreamDecoder, absolute_byte_offset: *mut u64, client_data: *mut os::raw::c_void) -> FLAC__StreamDecoderTellStatus
    where R: io::Read + io::Seek + SeekExt {
    let mut data = (client_data as *mut DecoderCallbackData<R>).as_mut().unwrap();
    if let Ok(pos) = data.input.tell() {
        *absolute_byte_offset = pos;
        return FLAC__StreamDecoderTellStatus::FLAC__STREAM_DECODER_TELL_STATUS_OK;
    }
    FLAC__StreamDecoderTellStatus::FLAC__STREAM_DECODER_TELL_STATUS_ERROR
}

unsafe extern "C" fn length_cb<R>(_: *const FLAC__StreamDecoder, stream_length: *mut u64, client_data: *mut os::raw::c_void) -> FLAC__StreamDecoderLengthStatus
    where R: io::Read + io::Seek + SeekExt {
    let mut data = (client_data as *mut DecoderCallbackData<R>).as_mut().unwrap();
    if let Ok(pos) = data.input.length() {
        *stream_length = pos;
        return FLAC__StreamDecoderLengthStatus::FLAC__STREAM_DECODER_LENGTH_STATUS_OK;
    }
    FLAC__StreamDecoderLengthStatus::FLAC__STREAM_DECODER_LENGTH_STATUS_ERROR
}

unsafe extern "C" fn eof_cb<R>(_: *const FLAC__StreamDecoder, client_data: *mut os::raw::c_void) -> i32
    where R: io::Read + io::Seek + SeekExt {
    let mut data = (client_data as *mut DecoderCallbackData<R>).as_mut().unwrap();
    if data.input.at_eof() {
        1
    } else {
        0
    }
}

unsafe extern "C" fn metadata_cb<R>(_: *const FLAC__StreamDecoder, metadata: *const FLAC__StreamMetadata, client_data: *mut os::raw::c_void)
    where R: io::Read + io::Seek + SeekExt {
    let mut data = (client_data as *mut DecoderCallbackData<R>).as_mut().unwrap();
    let mut meta = match data.meta.as_mut() {
        Some(meta) => meta,
        None => {
            warn!("picuture encountered after initialisation");
            return;
        },
    };

    match (*metadata).type_ {
        FLAC__METADATA_TYPE_VORBIS_COMMENT => {
            let comment = (*metadata).data.vorbis_comment.as_ref();
            let strings = slice::from_raw_parts(comment.comments, comment.num_comments as usize)
                .iter()
                .filter_map(|c| entry_as_str(c))
                .filter_map(|s| s.find('=').map(|i| (s, i)))
                .filter(|&(ref s, ref i)| s[*i..].trim().len() > 0);
            for (s, i) in strings {
                use id3::frame::Content;
                let (key, value) = s.split_at(i);
                let value = value[1..].trim();
                let (id, content) = match key.to_lowercase().replace(&[' ', '_'][..], "").as_str() {
                    "album" => ("TALB", Content::Text(value.to_string())),
                    "albumartist" => ("TPE2", Content::Text(value.to_string())),
                    "artist" => ("TPE1", Content::Text(value.to_string())),
                    "date" => ("TDRL", Content::Text(value.to_string())),
                    "disc"|"discnumber" => ("TPOS", Content::Text(value.to_string())),
                    "genre" => ("TCON", Content::Text(value.to_string())),
                    "software" => ("TSSE", Content::Text(value.to_string())),
                    "title" => ("TIT2", Content::Text(value.to_string())),
                    "track"|"tracknumber" => ("TRCK", Content::Text(value.to_string())),
                    "rating" => ("POPM", {
                        let rs = value.parse()
                            .ok()
                            .and_then(|n| match n {
                                0 => Some(0),
                                1 => Some(1),
                                2 => Some(64),
                                3 => Some(128),
                                4 => Some(196),
                                5 => Some(255),
                                _ => None,
                            })
                            .map(|n| {
                                let mut a = "Windows Media Player 9 Series\0_\0\0\0\0"
                                    .to_string()
                                    .into_bytes();
                                assert_eq!(a[30], '_' as u8);
                                a[30] = n;
                                Content::Unknown(a)
                            });
                        match rs {
                            Some(c) => c,
                            None => {
                                warn!("invalid value for rating: {}", value);
                                continue;
                            },
                        }
                    }),
                    unk => {
                        warn!("could not translate \"{}\" with value \"{}\" to id3", unk, value);
                        continue;
                    },
                };
                let mut frame = id3::Frame::new(id);
                frame.content = content;
                meta.tag.as_mut().unwrap().push(frame);
            }
        },
        FLAC__METADATA_TYPE_PICTURE => {
            let picture = (*metadata).data.picture.as_ref();
            let mime = match ffi::CStr::from_ptr(picture.mime_type).to_str() {
                Ok(s) => s,
                Err(err) => {
                    error!("{}", err);
                    return;
                },
            };
            let description = ffi::CStr::from_ptr(picture.description as *mut i8)
                .to_string_lossy()
                .to_string();
            use id3::frame::PictureType;
            let typ = match picture.type_ {
                FLAC__STREAM_METADATA_PICTURE_TYPE_FILE_ICON_STANDARD => PictureType::Icon,
                FLAC__STREAM_METADATA_PICTURE_TYPE_FILE_ICON => PictureType::OtherIcon,
                FLAC__STREAM_METADATA_PICTURE_TYPE_FRONT_COVER => PictureType::CoverFront,
                FLAC__STREAM_METADATA_PICTURE_TYPE_BACK_COVER => PictureType::CoverBack,
                FLAC__STREAM_METADATA_PICTURE_TYPE_LEAFLET_PAGE => PictureType::Leaflet,
                FLAC__STREAM_METADATA_PICTURE_TYPE_MEDIA => PictureType::Media,
                FLAC__STREAM_METADATA_PICTURE_TYPE_LEAD_ARTIST => PictureType::LeadArtist,
                FLAC__STREAM_METADATA_PICTURE_TYPE_ARTIST => PictureType::Artist,
                FLAC__STREAM_METADATA_PICTURE_TYPE_CONDUCTOR => PictureType::Conductor,
                FLAC__STREAM_METADATA_PICTURE_TYPE_BAND => PictureType::Band,
                FLAC__STREAM_METADATA_PICTURE_TYPE_COMPOSER => PictureType::Composer,
                FLAC__STREAM_METADATA_PICTURE_TYPE_LYRICIST => PictureType::Lyricist,
                FLAC__STREAM_METADATA_PICTURE_TYPE_RECORDING_LOCATION => PictureType::RecordingLocation,
                FLAC__STREAM_METADATA_PICTURE_TYPE_DURING_RECORDING => PictureType::DuringRecording,
                FLAC__STREAM_METADATA_PICTURE_TYPE_DURING_PERFORMANCE => PictureType::DuringPerformance,
                FLAC__STREAM_METADATA_PICTURE_TYPE_VIDEO_SCREEN_CAPTURE => PictureType::ScreenCapture,
                FLAC__STREAM_METADATA_PICTURE_TYPE_FISH => PictureType::BrightFish,
                FLAC__STREAM_METADATA_PICTURE_TYPE_ILLUSTRATION => PictureType::Illustration,
                FLAC__STREAM_METADATA_PICTURE_TYPE_BAND_LOGOTYPE => PictureType::BandLogo,
                FLAC__STREAM_METADATA_PICTURE_TYPE_PUBLISHER_LOGOTYPE => PictureType::PublisherLogo,
                _ => PictureType::Other,
            };
            let mut data = Vec::with_capacity(picture.data_length as usize);
            data.extend_from_slice(slice::from_raw_parts(picture.data, picture.data_length as usize));
            let mut frame = id3::Frame::new("APIC");
            frame.content = id3::frame::Content::Picture(id3::frame::Picture{
                mime_type: mime.to_string(),
                picture_type: typ,
                description: description,
                data: data,
            });
            meta.tag.as_mut().unwrap().push(frame);
        },
        _ => (),
    }
}

unsafe fn entry_as_str<'a>(entry: &'a FLAC__StreamMetadata_VorbisComment_Entry) -> Option<Cow<'a, str>> {
    if entry.length == 0 {
        return None;
    }
    let bytes = slice::from_raw_parts(entry.entry, entry.length as usize + 1);
    ffi::CStr::from_bytes_with_nul(bytes)
        .ok()
        .map(|cs| cs.to_string_lossy())
}


pub trait SeekExt: io::Read + io::Seek {
    fn length(&mut self) -> Result<u64, ()>;

    fn at_eof(&mut self) -> bool {
        self.tell()
            .and_then(|pos| self.length().map(|len| pos >= len))
            .unwrap_or(true)
    }

    fn tell(&mut self) -> Result<u64, ()> {
        match self.seek(io::SeekFrom::Current(0)) {
            Ok(pos) => Ok(pos),
            Err(_) => return Err(()),
        }
    }
}

impl SeekExt for fs::File {
    fn length(&mut self) -> Result<u64, ()> {
        match self.metadata() {
            Ok(meta) => Ok(meta.len()),
            Err(_) => Err(()),
        }
    }
}


#[derive(Debug)]
pub enum Error {
    IO(io::Error),
    ConstructionFailed,
    InitFailed(FLAC__StreamDecoderInitStatus),
    BadState(FLAC__StreamDecoderState),
    Unimplemented{
        known_length: bool,
        num_channels: u32,
        sample_size: u32,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::IO(ref err) => {
                write!(f, "IO: {}", err)
            },
            Error::ConstructionFailed => {
                write!(f, "Failed to construct decoder")
            },
            Error::InitFailed(status) => unsafe {
                let s = FLAC__StreamDecoderInitStatusString.offset(status as isize);
                let errstr = ffi::CStr::from_ptr(*s);
                write!(f, "Flac init failed: {}", errstr.to_str().unwrap())
            },
            Error::BadState(state) => unsafe {
                let s = FLAC__StreamDecoderStateString.offset(state as isize);
                let errstr = ffi::CStr::from_ptr(*s);
                write!(f, "Flac bad state: {}", errstr.to_str().unwrap())
            },
            Error::Unimplemented{ known_length: kl, num_channels: nc, sample_size: ss } => {
                write!(f, "Flac format not implemented: {} channels, {} bits, finite: {}", nc, ss, kl)
            },
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        "Flac error"
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            Error::IO(ref err) => Some(err),
            _ => None,
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Error {
        Error::IO(err)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    fn testfile() -> &'static path::Path {
        path::Path::new("testdata/Various Artists - Dark Sine of the Moon/01 - The B-Trees - Lucy in the Cloud with Sine Waves.flac")
    }

    #[test]
    fn read_file() {
        let (audio, _) = decode(fs::File::open(testfile()).unwrap()).unwrap();
        assert!(audio.is_seek());
        assert_eq!(44100, audio.sample_rate());
    }

    #[test]
    fn metadata() {
        let (_, meta) = decode(fs::File::open(path::Path::new(testfile())).unwrap()).unwrap();
        assert_eq!(44100, meta.sample_rate);
        assert_ne!(0, meta.num_samples.unwrap());
        let tag = meta.tag.unwrap();
        assert_eq!(tag.title().unwrap(), "Lucy in the Cloud with Sine Waves");
        assert_eq!(tag.artist().unwrap(), "The B-Trees");
        assert_eq!(tag.album().unwrap(), "Dark Sine of the Moon");
        assert_eq!(tag.date_released().unwrap(), id3::Timestamp::parse("1984").unwrap());
        assert_eq!(tag.album_artist().unwrap(), "Various Artists");
    }
}
