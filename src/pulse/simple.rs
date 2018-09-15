use super::*;
use sample;
use std::*;

fn sample_spec<F>(rate: u32) -> pa_sample_spec
where
    F: sample::Frame,
    F::Sample: sample::Sample + AsSampleFormat,
{
    assert!(F::n_channels() <= u8::MAX as usize);
    pa_sample_spec {
        format: F::Sample::sample_format(),
        rate,
        channels: F::n_channels() as u8,
    }
}

pub struct Connection<F>
where
    F: sample::Frame,
{
    conn: *mut pa_simple,
    phantom: marker::PhantomData<F>,
}

impl<F> Connection<F>
where
    F: sample::Frame,
    F::Sample: sample::Sample + AsSampleFormat,
{
    pub fn new(
        app_name: &str,
        stream_name: &str,
        rate: u32,
        dir: pa_stream_direction,
    ) -> Result<Connection<F>, Box<error::Error>> {
        let s = unsafe {
            let c_app_name = try!(ffi::CString::new(app_name));
            let c_stream_name = try!(ffi::CString::new(stream_name));
            let mut err_code = pa_error_code::PA_OK;
            let s = pa_simple_new(
                ptr::null(), // Use the default server.
                c_app_name.as_ptr(),
                dir,
                ptr::null(), // Use the default device.
                c_stream_name.as_ptr(),
                &sample_spec::<F>(rate),
                ptr::null(), // Use default channel map
                ptr::null(), // Use default buffering attributes.
                &mut err_code as *mut _ as *mut i32,
            );
            if err_code != pa_error_code::PA_OK {
                return Err(Box::from(PulseError(err_code)));
            }
            s
        };

        Ok(Connection {
            conn: s,
            phantom: marker::PhantomData,
        })
    }
}

impl<F> Connection<F>
where
    F: sample::Frame,
{
    /// Blocks until all written samples have been played.
    pub fn drain(&self) -> Result<(), PulseError> {
        let mut err_code = pa_error_code::PA_OK;
        unsafe {
            pa_simple_drain(self.conn, &mut err_code as *mut _ as *mut i32);
        }
        if err_code != pa_error_code::PA_OK {
            return Err(PulseError(err_code));
        }
        Ok(())
    }

    pub fn latency(&self) -> Result<time::Duration, PulseError> {
        let mut err_code = pa_error_code::PA_OK;
        let usecs =
            unsafe { pa_simple_get_latency(self.conn, &mut err_code as *mut _ as *mut i32) };
        if err_code != pa_error_code::PA_OK {
            return Err(PulseError(err_code));
        }
        Ok(time::Duration::new(
            usecs / 1_000_000_000,
            (usecs % 1_000_000_000) as u32,
        ))
    }
}

impl<F> io::Read for Connection<F>
where
    F: sample::Frame,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let mut err_code = pa_error_code::PA_OK;
        unsafe {
            pa_simple_read(
                self.conn,
                buf.as_mut_ptr() as _,
                buf.len(),
                &mut err_code as *mut _ as *mut i32,
            );
        }
        if err_code != pa_error_code::PA_OK {
            return Err(io::Error::new(io::ErrorKind::Other, PulseError(err_code)));
        }
        Ok(buf.len())
    }
}

impl<F> io::Write for Connection<F>
where
    F: sample::Frame,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut err_code = pa_error_code::PA_OK;
        unsafe {
            pa_simple_write(
                self.conn,
                buf.as_ptr() as _,
                buf.len(),
                &mut err_code as *mut _ as *mut i32,
            );
        }
        if err_code != pa_error_code::PA_OK {
            return Err(io::Error::new(io::ErrorKind::Other, PulseError(err_code)));
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut err_code = pa_error_code::PA_OK;
        unsafe {
            pa_simple_flush(self.conn, &mut err_code as *mut _ as *mut i32);
        }
        if err_code != pa_error_code::PA_OK {
            return Err(io::Error::new(io::ErrorKind::Other, PulseError(err_code)));
        }
        Ok(())
    }
}

impl<F> Drop for Connection<F>
where
    F: sample::Frame,
{
    fn drop(&mut self) {
        let _ = self.drain();
        unsafe {
            pa_simple_free(self.conn);
        }
    }
}

// Should be ok according to Pulse's documentation.
unsafe impl<F> Send for Connection<F> where F: sample::Frame {}

pub trait AsSampleFormat {
    fn sample_format() -> pa_sample_format;
}

impl AsSampleFormat for u8 {
    fn sample_format() -> pa_sample_format {
        pa_sample_format::PA_SAMPLE_U8
    }
}

#[cfg(target_endian = "little")]
mod endian {
    use super::*;
    impl AsSampleFormat for i16 {
        fn sample_format() -> pa_sample_format {
            pa_sample_format::PA_SAMPLE_S16LE
        }
    }
    impl AsSampleFormat for sample::I24 {
        fn sample_format() -> pa_sample_format {
            assert_eq!(4, mem::size_of::<Self>());
            pa_sample_format::PA_SAMPLE_S24_32LE
        }
    }
    impl AsSampleFormat for f32 {
        fn sample_format() -> pa_sample_format {
            pa_sample_format::PA_SAMPLE_FLOAT32LE
        }
    }
}

#[cfg(target_endian = "big")]
mod endian {
    use super::*;
    impl AsSampleFormat for i16 {
        fn sample_format() -> pa_sample_format {
            pa_sample_format::PA_SAMPLE_S16BE
        }
    }
    impl AsSampleFormat for sample::I24 {
        fn sample_format() -> pa_sample_format {
            assert_eq!(4, mem::size_of::<Self>());
            pa_sample_format::PA_SAMPLE_S24_32BE
        }
    }
    impl AsSampleFormat for f32 {
        fn sample_format() -> pa_sample_format {
            pa_sample_format::PA_SAMPLE_FLOAT32BE
        }
    }
}
