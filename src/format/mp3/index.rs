use std::*;

#[derive(Copy, Clone, Debug)]
pub enum MpegLayer {
    L1,
    L2,
    L3,
}

#[derive(Copy, Clone, Debug)]
pub enum MpegVersion {
    V1,
    V2,
    V25,
}

fn find_bitrate(index: u8, version: MpegVersion, layer: MpegLayer) -> Result<Option<u32>, Error> {
    use self::MpegLayer::*;
    use self::MpegVersion::*;
    Ok(Some(
        match (index, version, layer) {
            // bits   V1,L1   V1,L2   V1,L3   V2,L1   V2,L2,L3
            // 0001   32      32      32      32      8
            (0b0001, V2, L2) => 8,
            (0b0001, V2, L3) => 8,
            (0b0001, _, _) => 32,
            // 0010   64      48      40      48      16
            (0b0010, V1, L1) => 64,
            (0b0010, V1, L2) => 48,
            (0b0010, V1, L3) => 40,
            (0b0010, V2, L1) => 48,
            (0b0010, V2, L2) => 16,
            (0b0010, V2, L3) => 16,
            // 0011   96      56      48      56      24
            (0b0011, V1, L1) => 96,
            (0b0011, V1, L2) => 56,
            (0b0011, V1, L3) => 48,
            (0b0011, V2, L1) => 56,
            (0b0011, V2, L2) => 24,
            (0b0011, V2, L3) => 24,
            // 0100   128     64      56      64      32
            (0b0100, V1, L1) => 128,
            (0b0100, V1, L2) => 64,
            (0b0100, V1, L3) => 56,
            (0b0100, V2, L1) => 64,
            (0b0100, V2, L2) => 32,
            (0b0100, V2, L3) => 32,
            // 0101   160     80      64      80      40
            (0b0101, V1, L1) => 160,
            (0b0101, V1, L2) => 80,
            (0b0101, V1, L3) => 64,
            (0b0101, V2, L1) => 80,
            (0b0101, V2, L2) => 40,
            (0b0101, V2, L3) => 40,
            // 0110   192     96      80      96      48
            (0b0110, V1, L1) => 192,
            (0b0110, V1, L2) => 96,
            (0b0110, V1, L3) => 80,
            (0b0110, V2, L1) => 96,
            (0b0110, V2, L2) => 48,
            (0b0110, V2, L3) => 48,
            // 0111   224     112     96      112     56
            (0b0111, V1, L1) => 224,
            (0b0111, V1, L2) => 112,
            (0b0111, V1, L3) => 96,
            (0b0111, V2, L1) => 112,
            (0b0111, V2, L2) => 56,
            (0b0111, V2, L3) => 56,
            // 1000   256     128     112     128     64
            (0b1000, V1, L1) => 256,
            (0b1000, V1, L2) => 128,
            (0b1000, V1, L3) => 112,
            (0b1000, V2, L1) => 128,
            (0b1000, V2, L2) => 64,
            (0b1000, V2, L3) => 64,
            // 1001   288     160     128     144     80
            (0b1001, V1, L1) => 288,
            (0b1001, V1, L2) => 160,
            (0b1001, V1, L3) => 128,
            (0b1001, V2, L1) => 144,
            (0b1001, V2, L2) => 80,
            (0b1001, V2, L3) => 80,
            // 1010   320     192     160     160     96
            (0b1010, V1, L1) => 320,
            (0b1010, V1, L2) => 192,
            (0b1010, V1, L3) => 160,
            (0b1010, V2, L1) => 160,
            (0b1010, V2, L2) => 96,
            (0b1010, V2, L3) => 96,
            // 1011   352     224     192     176     112
            (0b1011, V1, L1) => 352,
            (0b1011, V1, L2) => 224,
            (0b1011, V1, L3) => 192,
            (0b1011, V2, L1) => 176,
            (0b1011, V2, L2) => 112,
            (0b1011, V2, L3) => 112,
            // 1100   384     256     224     192     128
            (0b1100, V1, L1) => 384,
            (0b1100, V1, L2) => 256,
            (0b1100, V1, L3) => 224,
            (0b1100, V2, L1) => 192,
            (0b1100, V2, L2) => 128,
            (0b1100, V2, L3) => 128,
            // 1101   416     320     256     224     144
            (0b1101, V1, L1) => 416,
            (0b1101, V1, L2) => 320,
            (0b1101, V1, L3) => 256,
            (0b1101, V2, L1) => 224,
            (0b1101, V2, L2) => 144,
            (0b1101, V2, L3) => 144,
            // 1110   448     384     320     256     160
            (0b1110, V1, L1) => 448,
            (0b1110, V1, L2) => 384,
            (0b1110, V1, L3) => 320,
            (0b1110, V2, L1) => 256,
            (0b1110, V2, L2) => 160,
            (0b1110, V2, L3) => 160,
            // 0000   free    free    free    free    free
            (0b0000, _, _) => return Ok(None),
            // 1111   bad     bad     bad     bad     bad
            (0b1111, _, _) => return Ok(None),
            (index, version, layer) => {
                return Err(Error::UnknownBitrate {
                    index,
                    layer,
                    version,
                })
            }
        } * 1000,
    ))
}

/// Find and seek to the start of the next frame header.
/// MP3 frame headers start with a sequence of 11 bits set to 1.
pub fn find_stream<R>(input: &mut R) -> Result<(), Error>
where
    R: io::Read + io::Seek,
{
    loop {
        let block_offset = input.seek(io::SeekFrom::Current(0))?;
        let mut buf = [0; 8192];
        let num_read = input.read(&mut buf)?;
        if num_read <= 1 {
            return Err(Error::MissingSync);
        }
        for (b, i) in buf[..num_read].windows(2).zip(block_offset..) {
            if b[0] == 0xff && b[1] & 0xe0 == 0xe0 {
                input.seek(io::SeekFrom::Start(i))?;
                return Ok(());
            }
        }
        // Go back one byte in case the two frame sync bytes are on a buffer size boundary.
        input.seek(io::SeekFrom::Current(-1))?;
    }
}

pub struct Frame {
    /// Absolute byte offset in the file.
    pub offset: u64,
    /// Length of the frame in bytes.
    pub length: u32,
    /// The number of combined audio samples. Altough not available in this struct, multiply by the
    /// number of channels to get the total number of samples.
    pub num_samples: u16,
    /// The number of samples preceeding this frame.
    pub sample_offset: u64,
}

pub struct FrameIndex {
    /// Frame offsets and lengths.
    pub frames: Vec<Frame>,
}

impl FrameIndex {
    pub fn read<R>(input: &mut R) -> Result<FrameIndex, Error>
    where
        R: io::Read + io::Seek,
    {
        // http://mpgedit.org/mpgedit/mpeg_format/mpeghdr.htm

        find_stream(input)?;

        let mut sample_count = 0;
        let mut frames = Vec::new();
        loop {
            let header_offset = input.seek(io::SeekFrom::Current(0))?;
            let mut header = [0; 4];
            if input.read(&mut header)? < header.len() {
                break;
            }
            if header[0] != 0xff || header[1] & 0xe0 != 0xe0 {
                break; // Lost sync or other data after the stream.
            }

            use self::MpegLayer::*;
            use self::MpegVersion::*;
            let version = match header[1] >> 3 & 0x03 {
                0b00 => V25,
                0b01 => break, // Reserved.
                0b10 => V2,
                0b11 => V1,
                _ => unreachable!(),
            };
            let layer = match header[1] >> 1 & 0x03 {
                0b00 => break, // Reserved.
                0b01 => L3,
                0b10 => L2,
                0b11 => L1,
                _ => unreachable!(),
            };
            let bitrate = match find_bitrate(header[2] >> 4 & 0x0f, version, layer) {
                Ok(Some(br)) => br,
                _ => break,
            };
            let sample_rate = match (header[2] >> 2 & 0x03, version) {
                (0b11, _) => break, // Reserved.
                (0b00, V1) => 44100,
                (0b01, V1) => 48000,
                (0b10, V1) => 32000,
                (0b00, V2) => 22050,
                (0b01, V2) => 24000,
                (0b10, V2) => 16000,
                (0b00, V25) => 11025,
                (0b01, V25) => 12000,
                (0b10, V25) => 8000,
                _ => unreachable!(),
            };
            let has_padding = header[2] >> 1 & 1 == 1;
            let num_channels = match header[3] >> 6 & 0x03 {
                0b00 => 2, // Stereo
                // Joint stereo. Actually 2 channels, but samples / channels gives funny results.
                0b01 => 1,
                0b10 => 2, // Dual channel
                0b11 => 1, // Single channel
                _ => unreachable!(),
            };

            let num_samples = match layer {
                L1 => 384,
                L2 | L3 => 1152,
            };
            let padding = match (has_padding, layer) {
                (false, _) => 0,
                (_, L1) => 4,
                (_, L2) | (_, L3) => 1,
            };
            let frame_length = match layer {
                L1 => (12 * bitrate / sample_rate + padding) * 4,
                L2 | L3 => 144 * bitrate / sample_rate + padding,
            };

            let next = input.seek(io::SeekFrom::Start(header_offset + u64::from(frame_length)))?;
            assert!(next > header_offset);

            frames.push(Frame {
                offset: header_offset,
                length: frame_length,
                num_samples: num_samples / num_channels,
                sample_offset: sample_count / u64::from(num_channels),
            });
            sample_count += u64::from(num_samples);
        }

        if frames.is_empty() {
            return Err(Error::MissingSync);
        }
        Ok(FrameIndex { frames })
    }

    pub fn num_samples(&self) -> u64 {
        let frame = self.frames.last().unwrap();
        frame.sample_offset + u64::from(frame.num_samples)
    }

    pub fn frame_for_sample(&self, nth_sample: u64) -> Option<usize> {
        self.frames
            .binary_search_by(|frame| {
                if frame.sample_offset > nth_sample {
                    cmp::Ordering::Greater
                } else if frame.sample_offset + (u64::from(frame.num_samples)) < nth_sample {
                    cmp::Ordering::Less
                } else {
                    cmp::Ordering::Equal
                }
            }).ok()
    }
}

#[derive(Debug)]
pub enum Error {
    IO(io::Error),
    MissingSync,
    UnknownBitrate {
        index: u8,
        layer: MpegLayer,
        version: MpegVersion,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            Error::IO(ref err) => write!(f, "IO: {}", err),
            Error::MissingSync => write!(f, "Frame sync not found"),
            Error::UnknownBitrate {
                index,
                layer,
                version,
            } => write!(
                f,
                "Unknown bitrate: index={:4b}, layer={:?}, version={:?}",
                index, layer, version
            ),
        }
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        "Indexing error"
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

    #[test]
    fn find_stream_start() {
        let mut cur = io::Cursor::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xe0, 0, 0, 0, 0]);
        find_stream(&mut cur).unwrap();
        assert_eq!(10, cur.position());
    }

    #[test]
    fn find_stream_repeated() {
        let mut cur = io::Cursor::new([0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff, 0xe0, 0, 0, 0, 0]);
        find_stream(&mut cur).unwrap();
        assert_eq!(10, cur.position());
        find_stream(&mut cur).unwrap();
        assert_eq!(10, cur.position());
        find_stream(&mut cur).unwrap();
        assert_eq!(10, cur.position());
    }
}

#[cfg(all(test, feature = "unstable"))]
mod benchmarks {
    extern crate test;
    use super::*;
    use std::io::Seek;

    #[bench]
    fn find_stream_start(b: &mut test::Bencher) {
        b.iter(|| {
            let mut file = fs::File::open("testdata/10s_440hz_320cbr_stereo.mp3").unwrap();
            find_stream(&mut file).unwrap();
        });
    }

    #[bench]
    fn build_index(b: &mut test::Bencher) {
        let mut file = fs::File::open("testdata/10s_440hz_320cbr_stereo.mp3").unwrap();
        find_stream(&mut file).unwrap();
        let stream_start = file.seek(io::SeekFrom::Current(0)).unwrap();

        b.iter(|| {
            let mut file = fs::File::open("testdata/10s_440hz_320cbr_stereo.mp3").unwrap();
            file.seek(io::SeekFrom::Start(stream_start)).unwrap();
            FrameIndex::read(&mut file).unwrap();
        });
    }
}
