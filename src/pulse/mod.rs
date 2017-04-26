use std::*;
use std::io::{Read, Write};
use libpulse_sys::*;
use sample;
use ::audio;

pub mod simple;
pub use self::simple::*;


pub struct Source<F: sample::Frame> {
    conn: Connection<F>,
    rate: u32,
}

impl<F> Source<F>
    where F: sample::Frame {
    pub fn connection(&self) -> &Connection<F> {
        &self.conn
    }
}

impl<F> iter::Iterator for Source<F>
    where F: sample::Frame {
    type Item = F;
    fn next(&mut self) -> Option<Self::Item> {
        unsafe {
            debug_assert_eq!(mem::size_of::<F>(), F::n_channels() * mem::size_of::<F::Sample>());
            let mut frame: F = mem::uninitialized();
            let buf = slice::from_raw_parts_mut(mem::transmute::<&F, *mut u8>(&mut frame), mem::size_of::<F>());
            self.conn.read(buf).unwrap();
            Some(frame)
        }
    }
}

impl<F> audio::Source for Source<F>
    where F: sample::Frame {
    fn sample_rate(&self) -> u32 {
        self.rate
    }
}

pub fn source<F, S>(app_name: &str, rate: u32) -> Result<Source<F>, Box<error::Error>>
    where F: sample::Frame<Sample=S>,
          S: sample::Sample + AsSampleFormat {
    Connection::new(app_name, "source", rate, pa_stream_direction::PA_STREAM_RECORD)
        .map(|c| Source { conn: c, rate: rate })
}

pub struct Sink<F: sample::Frame> {
    conn: io::BufWriter<Connection<F>>,
    rate: u32,
}

impl<F> Sink<F>
    where F: sample::Frame {
    pub fn connection(&self) -> &Connection<F> {
        self.conn.get_ref()
    }
}

impl<F> audio::Sink<F> for Sink<F>
    where F: sample::Frame {
    fn write_frame(&mut self, frame: F) -> Result<(), Box<error::Error + Send>> {
        unsafe {
            debug_assert_eq!(mem::size_of::<F>(), F::n_channels() * mem::size_of::<F::Sample>());
            let buf = slice::from_raw_parts(mem::transmute::<&F, *const u8>(&frame), mem::size_of::<F>());
            match self.conn.write(buf) {
                // From<T> does not work well with T + Send :(
                Ok(_) => (),
                Err(ioerr) => return Err(Box::new(ioerr)),
            };
            Ok(())
        }
    }

    fn sample_rate(&self) -> u32 {
        self.rate
    }
}

impl<F> Drop for Sink<F>
    where F: sample::Frame {
    fn drop(&mut self) {
        let _ = self.conn.get_ref().drain();
    }
}

pub fn sink<F>(app_name: &str, stream_name: &str, rate: u32) -> Result<Sink<F>, Box<error::Error>>
    where F: sample::Frame,
          F::Sample: sample::Sample + AsSampleFormat {
    Connection::new(app_name, stream_name, rate, pa_stream_direction::PA_STREAM_PLAYBACK)
        .map(|c| Sink { conn: io::BufWriter::new(c), rate: rate })
}


#[derive(Debug)]
pub struct PulseError(pa_error_code);

impl fmt::Display for PulseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        unsafe {
            let errstr = ffi::CStr::from_ptr(pa_strerror(self.0 as i32));
            write!(f, "Pulse error: {}", errstr.to_str().unwrap())
        }
    }
}

impl error::Error for PulseError {
    fn description(&self) -> &str {
        unsafe {
            let errstr = ffi::CStr::from_ptr(pa_strerror(self.0 as i32));
            errstr.to_str().unwrap()
        }
    }
    fn cause(&self) -> Option<&error::Error> {
        None
    }
}
