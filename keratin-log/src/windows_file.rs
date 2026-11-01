//! Positional operations must not move the segment writer's shared cursor.
//! Windows seek_read/seek_write move that cursor even through a cloned handle.
//! Reopening the file gives each operation an independent cursor while retaining
//! the same file identity across renames. Sync calls remain on the original file.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::os::windows::io::{AsRawHandle, FromRawHandle};
use windows_sys::Win32::Foundation::{GENERIC_READ, GENERIC_WRITE, INVALID_HANDLE_VALUE};
use windows_sys::Win32::Storage::FileSystem::{
    BY_HANDLE_FILE_INFORMATION, FILE_SHARE_DELETE, FILE_SHARE_READ, FILE_SHARE_WRITE,
    GetFileInformationByHandle, ReOpenFile,
};

fn reopen(file: &File, access: u32) -> io::Result<File> {
    // The input is live for the call. A successful result is a new owned handle.
    let handle = unsafe {
        ReOpenFile(
            file.as_raw_handle(),
            access,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            0,
        )
    };
    if handle == INVALID_HANDLE_VALUE {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_handle(handle) })
}

pub(crate) trait FileExt {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize>;
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()>;
    fn write_all_at(&self, buf: &[u8], offset: u64) -> io::Result<()>;
}

impl FileExt for File {
    fn read_at(&self, buf: &mut [u8], offset: u64) -> io::Result<usize> {
        let mut file = reopen(self, GENERIC_READ)?;
        file.seek(SeekFrom::Start(offset))?;
        file.read(buf)
    }

    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        let mut file = reopen(self, GENERIC_READ)?;
        file.seek(SeekFrom::Start(offset))?;
        file.read_exact(buf)
    }

    fn write_all_at(&self, buf: &[u8], offset: u64) -> io::Result<()> {
        let mut file = reopen(self, GENERIC_WRITE)?;
        file.seek(SeekFrom::Start(offset))?;
        file.write_all(buf)
    }
}

pub(crate) fn number_of_links(file: &File) -> io::Result<u64> {
    Ok(u64::from(file_information(file)?.nNumberOfLinks))
}

#[cfg(feature = "io-fault-injection")]
pub(crate) fn identity(file: &File) -> io::Result<(u64, u64)> {
    let info = file_information(file)?;
    Ok((
        u64::from(info.dwVolumeSerialNumber),
        (u64::from(info.nFileIndexHigh) << 32) | u64::from(info.nFileIndexLow),
    ))
}

fn file_information(file: &File) -> io::Result<BY_HANDLE_FILE_INFORMATION> {
    let mut info: BY_HANDLE_FILE_INFORMATION = unsafe { std::mem::zeroed() };
    // The output is initialized only on success; the input handle stays live.
    if unsafe { GetFileInformationByHandle(file.as_raw_handle(), &mut info) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(info)
}
