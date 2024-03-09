use windows::Win32::{
    Foundation::ERROR_NO_UNICODE_TRANSLATION,
    System::Com::{IStream, STREAM_SEEK_END, STREAM_SEEK_SET},
};

pub fn read_com_stream(stream: &IStream) -> Result<Vec<u8>, windows::core::Error> {
    let mut size = 0;
    unsafe {
        stream.Seek(0, STREAM_SEEK_END, Some(&mut size))?;
        stream.Seek(0, STREAM_SEEK_SET, None)?;
        let mut data = Vec::with_capacity(size as usize);
        let mut n_read_bytes = 0;
        stream
            .Read(
                data.as_mut_ptr() as *mut _,
                data.capacity() as u32,
                Some(&mut n_read_bytes),
            )
            .ok()?;
        data.set_len(n_read_bytes as usize);
        Ok(data)
    }
}

/// Copies the contents of a COM stream to a string.
/// If the stream contains invalid UTF-8, an error will be returned.
pub fn copy_com_stream_to_string(
    dest: &mut String,
    stream: &IStream,
) -> Result<(), windows::core::Error> {
    let mut size = 0;
    unsafe {
        stream.Seek(0, STREAM_SEEK_END, Some(&mut size))?;
        stream.Seek(0, STREAM_SEEK_SET, None)?;
        dest.clear();
        dest.reserve(size as usize);
        let mut n_read_bytes = 0;
        stream
            .Read(
                dest.as_mut_ptr() as *mut _,
                dest.capacity() as u32,
                Some(&mut n_read_bytes),
            )
            .ok()?;
        if std::str::from_utf8(std::slice::from_raw_parts(
            dest.as_ptr(),
            n_read_bytes as usize,
        ))
        .is_err()
        {
            return Err(ERROR_NO_UNICODE_TRANSLATION.into());
        }
        dest.as_mut_vec().set_len(n_read_bytes as usize);
        Ok(())
    }
}
