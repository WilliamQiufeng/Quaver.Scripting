#[cfg(any(target_os = "linux", target_os = "macos"))]
use memmap2::{MmapMut, MmapOptions};

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub type MappedMemory = MmapMut;

#[cfg(target_os = "linux")]
pub fn open(path: &str, size: usize) -> std::io::Result<MappedMemory> {
    use std::fs::OpenOptions;

    let file = OpenOptions::new().read(true).write(true).open(path)?;
    unsafe { MmapOptions::new().len(size).map_mut(&file) }
}

#[cfg(target_os = "macos")]
pub fn open(path: &str, size: usize) -> std::io::Result<MappedMemory> {
    use std::ffi::CString;
    use std::fs::File;
    use std::os::fd::{FromRawFd, OwnedFd};

    let name = CString::new(path)?;
    let fd = unsafe { libc::shm_open(name.as_ptr(), libc::O_RDWR, 0) };

    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }

    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    let file = File::from(owned);
    unsafe { MmapOptions::new().len(size).map_mut(&file) }
}

#[cfg(target_os = "windows")]
mod windows {
    use std::{
        io,
        ops::{Deref, DerefMut},
        ptr::NonNull,
        slice,
    };

    use windows_sys::Win32::{
        Foundation::{CloseHandle, HANDLE},
        System::Memory::{
            FILE_MAP_READ, FILE_MAP_WRITE, MEMORY_MAPPED_VIEW_ADDRESS, MapViewOfFile,
            OpenFileMappingW, UnmapViewOfFile,
        },
    };

    pub struct MappedMemory {
        handle: HANDLE,
        view: MEMORY_MAPPED_VIEW_ADDRESS,
        ptr: NonNull<u8>,
        len: usize,
    }

    impl MappedMemory {
        pub fn open(name: &str, size: usize) -> io::Result<Self> {
            let name = to_wide_null(name);
            let access = FILE_MAP_READ | FILE_MAP_WRITE;

            let handle = unsafe { OpenFileMappingW(access, 0, name.as_ptr()) };
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }

            let view = unsafe { MapViewOfFile(handle, access, 0, 0, size) };
            let Some(ptr) = NonNull::new(view.Value.cast::<u8>()) else {
                unsafe {
                    CloseHandle(handle);
                }
                return Err(io::Error::last_os_error());
            };

            Ok(Self {
                handle,
                view,
                ptr,
                len: size,
            })
        }

        pub fn len(&self) -> usize {
            self.len
        }

        pub fn as_ptr(&self) -> *const u8 {
            self.ptr.as_ptr()
        }
    }

    impl Deref for MappedMemory {
        type Target = [u8];

        fn deref(&self) -> &Self::Target {
            unsafe { slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
        }
    }

    impl DerefMut for MappedMemory {
        fn deref_mut(&mut self) -> &mut Self::Target {
            unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
        }
    }

    impl Drop for MappedMemory {
        fn drop(&mut self) {
            unsafe {
                UnmapViewOfFile(self.view);
                CloseHandle(self.handle);
            }
        }
    }

    unsafe impl Send for MappedMemory {}
    unsafe impl Sync for MappedMemory {}

    fn to_wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }
}

#[cfg(target_os = "windows")]
pub use windows::MappedMemory;

#[cfg(target_os = "windows")]
pub fn open(path: &str, size: usize) -> std::io::Result<MappedMemory> {
    MappedMemory::open(path, size)
}
