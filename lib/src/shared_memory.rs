use std::{
    cmp,
    fs::File,
    mem::{align_of, size_of},
    sync::atomic::{AtomicU64, Ordering},
};

use memmap2::{MmapMut, MmapOptions};

#[repr(C)]
pub struct SharedMemoryLayout {
    magic: u32,
    version: u32,

    host_frame: AtomicU64,
    worker_frame: AtomicU64,

    host_write: AtomicU64,
    host_read: AtomicU64,

    worker_write: AtomicU64,
    worker_read: AtomicU64,
}

pub struct SharedMemoryInstance {
    mmap: MmapMut,
}

impl SharedMemoryInstance {
    pub fn layout(&self) -> &SharedMemoryLayout {
        assert!(self.mmap.len() >= size_of::<SharedMemoryLayout>());
        let ptr = self.mmap.as_ptr();
        assert_eq!(ptr.align_offset(align_of::<SharedMemoryLayout>()), 0);
        unsafe { &*(ptr.cast::<SharedMemoryLayout>()) }
    }

    pub fn from_file(file: &File, size: usize) -> std::io::Result<Self> {
        assert!(size >= size_of::<SharedMemoryLayout>());
        let mmap = open(file, size)?;
        Ok(Self { mmap })
    }

    pub fn read(&mut self, output: &mut [u8]) -> usize {
        let layout = self.layout();
        let (host_buffer, _) = split_payload(self.payload());

        read_ring(&layout.host_write, &layout.host_read, host_buffer, output)
    }

    pub fn write(&mut self, input: &[u8]) -> usize {
        let (layout, payload) = self.layout_payload_mut();
        let (worker_buffer, _) = split_payload_mut(payload);

        write_ring(
            &layout.worker_write,
            &layout.worker_read,
            worker_buffer,
            input,
        )
    }

    fn layout_payload_mut(&mut self) -> (&SharedMemoryLayout, &mut [u8]) {
        assert!(self.mmap.len() >= size_of::<SharedMemoryLayout>());
        let ptr = self.mmap.as_ptr();
        assert_eq!(ptr.align_offset(align_of::<SharedMemoryLayout>()), 0);
        let layout_size = size_of::<SharedMemoryLayout>();
        let (_, payload) = self.mmap.split_at_mut(layout_size);
        let layout = unsafe { &*(ptr.cast::<SharedMemoryLayout>()) };
        (layout, payload)
    }

    fn payload(&self) -> &[u8] {
        let layout_size = size_of::<SharedMemoryLayout>();
        let (_, payload) = self.mmap.split_at(layout_size);
        payload
    }
}

fn open(file: &File, size: usize) -> std::io::Result<MmapMut> {
    unsafe { MmapOptions::new().len(size).map_mut(file) }
}

fn split_payload_mut(payload: &mut [u8]) -> (&mut [u8], &mut [u8]) {
    let capacity = payload.len() / 2;
    let (host_buffer, rest) = payload.split_at_mut(capacity);
    let (worker_buffer, _) = rest.split_at_mut(capacity);

    (host_buffer, worker_buffer)
}

fn split_payload(payload: &[u8]) -> (&[u8], &[u8]) {
    let capacity = payload.len() / 2;
    let (host_buffer, rest) = payload.split_at(capacity);
    let (worker_buffer, _) = rest.split_at(capacity);

    (host_buffer, worker_buffer)
}

fn read_ring(write: &AtomicU64, read: &AtomicU64, buffer: &[u8], output: &mut [u8]) -> usize {
    if buffer.is_empty() || output.is_empty() {
        return 0;
    }

    let write_pos = write.load(Ordering::Acquire);
    let read_pos = read.load(Ordering::Relaxed);
    let available = (write_pos.saturating_sub(read_pos) as usize).min(buffer.len());
    let count = cmp::min(available, output.len());

    copy_from_ring(output, buffer, read_pos as usize % buffer.len(), count);
    read.store(read_pos + count as u64, Ordering::Release);

    count
}

fn write_ring(write: &AtomicU64, read: &AtomicU64, buffer: &mut [u8], input: &[u8]) -> usize {
    if buffer.is_empty() || input.is_empty() {
        return 0;
    }

    let write_pos = write.load(Ordering::Relaxed);
    let read_pos = read.load(Ordering::Acquire);
    let used = (write_pos.saturating_sub(read_pos) as usize).min(buffer.len());
    let available = buffer.len() - used;
    let count = cmp::min(available, input.len());

    copy_to_ring(buffer, write_pos as usize % buffer.len(), &input[..count]);
    write.store(write_pos + count as u64, Ordering::Release);

    count
}

fn copy_from_ring(output: &mut [u8], buffer: &[u8], start: usize, count: usize) {
    let first_count = cmp::min(count, buffer.len() - start);
    let second_count = count - first_count;

    output[..first_count].copy_from_slice(&buffer[start..start + first_count]);
    output[first_count..count].copy_from_slice(&buffer[..second_count]);
}

fn copy_to_ring(buffer: &mut [u8], start: usize, input: &[u8]) {
    let first_count = cmp::min(input.len(), buffer.len() - start);
    let second_count = input.len() - first_count;

    buffer[start..start + first_count].copy_from_slice(&input[..first_count]);
    buffer[..second_count].copy_from_slice(&input[first_count..]);
}
