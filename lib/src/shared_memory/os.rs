use memmap2::{MmapMut, MmapOptions};

#[cfg(target_os = "linux")]
pub fn open(path: &str, size: usize) -> std::io::Result<MmapMut> {
    use std::fs::OpenOptions;

    let file = OpenOptions::new().read(true).write(true).open(path)?;
    unsafe { MmapOptions::new().len(size).map_mut(&file) }
}

#[cfg(target_os = "macos")]
pub fn open(path: &str, size: usize) -> std::io::Result<MmapMut> {
    use std::ffi::CString;
    use std::os::fd::{FromRawFd, OwnedFd};
    let name = CString::new(path)?;

    let fd = unsafe { libc::shm_open(name.as_ptr(), libc::O_RDWR, 0) };

    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }

    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    let file = File::from(owned);
    unsafe { MmapOptions::new().len(size).map_mut(&file) }
}

#[cfg(target_os = "windows")]
pub fn open(path: &str, size: usize) -> std::io::Result<MmapMut> {
    todo!()
}