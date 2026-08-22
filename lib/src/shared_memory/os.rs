#[cfg(any(target_os = "linux", target_os = "macos"))]
use memmap2::{Mmap, MmapMut, MmapOptions};

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub type MappedMemory = Mmap;

#[cfg(any(target_os = "linux", target_os = "macos"))]
pub type MappedMemoryMut = MmapMut;

#[cfg(target_os = "linux")]
pub fn open(path: &str, size: usize) -> std::io::Result<MappedMemory> {
    use std::fs::File;

    let file = File::open(path)?;
    unsafe { MmapOptions::new().len(size).map(&file) }
}

#[cfg(target_os = "linux")]
pub fn open_mut(path: &str, size: usize) -> std::io::Result<MappedMemoryMut> {
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
    let fd = unsafe { libc::shm_open(name.as_ptr(), libc::O_RDONLY, 0) };

    if fd < 0 {
        return Err(std::io::Error::last_os_error());
    }

    let owned = unsafe { OwnedFd::from_raw_fd(fd) };
    let file = File::from(owned);
    unsafe { MmapOptions::new().len(size).map(&file) }
}

#[cfg(target_os = "macos")]
pub fn open_mut(path: &str, size: usize) -> std::io::Result<MappedMemoryMut> {
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
        marker::PhantomData,
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

    pub struct ReadOnly;
    pub struct ReadWrite;

    pub struct MappedMemoryBase<Access> {
        handle: HANDLE,
        view: MEMORY_MAPPED_VIEW_ADDRESS,
        ptr: NonNull<u8>,
        len: usize,
        _access: PhantomData<Access>,
    }

    pub type MappedMemory = MappedMemoryBase<ReadOnly>;
    pub type MappedMemoryMut = MappedMemoryBase<ReadWrite>;

    impl MappedMemory {
        pub fn open(name: &str, size: usize) -> io::Result<Self> {
            open_mapping(name, size, FILE_MAP_READ)
        }
    }

    impl MappedMemoryMut {
        pub fn open(name: &str, size: usize) -> io::Result<Self> {
            open_mapping(name, size, FILE_MAP_READ | FILE_MAP_WRITE)
        }
    }

    impl<Access> MappedMemoryBase<Access> {
        pub fn len(&self) -> usize {
            self.len
        }

        pub fn as_ptr(&self) -> *const u8 {
            self.ptr.as_ptr()
        }
    }

    impl<Access> Deref for MappedMemoryBase<Access> {
        type Target = [u8];

        fn deref(&self) -> &Self::Target {
            unsafe { slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
        }
    }

    impl DerefMut for MappedMemoryMut {
        fn deref_mut(&mut self) -> &mut Self::Target {
            unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
        }
    }

    impl<Access> Drop for MappedMemoryBase<Access> {
        fn drop(&mut self) {
            unsafe {
                UnmapViewOfFile(self.view);
                CloseHandle(self.handle);
            }
        }
    }

    unsafe impl<Access> Send for MappedMemoryBase<Access> {}
    unsafe impl<Access> Sync for MappedMemoryBase<Access> {}

    fn open_mapping<Access>(
        name: &str,
        size: usize,
        access: u32,
    ) -> io::Result<MappedMemoryBase<Access>> {
        let name = to_wide_null(name);

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

        Ok(MappedMemoryBase {
            handle,
            view,
            ptr,
            len: size,
            _access: PhantomData,
        })
    }

    fn to_wide_null(value: &str) -> Vec<u16> {
        value.encode_utf16().chain(Some(0)).collect()
    }
}

#[cfg(target_os = "windows")]
pub use windows::{MappedMemory, MappedMemoryMut};

#[cfg(target_os = "windows")]
pub fn open(path: &str, size: usize) -> std::io::Result<MappedMemory> {
    MappedMemory::open(path, size)
}

#[cfg(target_os = "windows")]
pub fn open_mut(path: &str, size: usize) -> std::io::Result<MappedMemoryMut> {
    MappedMemoryMut::open(path, size)
}
