use std::{
    cmp,
    fs::File,
    marker::PhantomData,
    mem::{align_of, size_of},
    sync::atomic::{AtomicU64, Ordering},
};

use memmap2::{MmapMut, MmapOptions};

#[repr(C)]
struct SharedMemoryLayout {
    magic: u32,
    version: u32,
    channel_size: u64,

    host_to_worker: SharedMemoryChannel<HostToWorker>,
    worker_to_host: SharedMemoryChannel<WorkerToHost>,
}

pub struct SharedMemoryInstance {
    mmap: MmapMut,
}

trait ChannelDirection {}
struct HostToWorker;
struct WorkerToHost;
impl ChannelDirection for HostToWorker {}
impl ChannelDirection for WorkerToHost {}

#[repr(C)]
struct SharedMemoryChannel<D: ChannelDirection> {
    offset: AtomicU64,
    write: AtomicU64,
    read: AtomicU64,
    _phantom: PhantomData<D>,
}

impl<D: ChannelDirection> SharedMemoryChannel<D> {
    fn find_buffer<'a>(&'a self, buffer: &'a [u8], channel_size: usize) -> Option<&'a [u8]> {
        let offset = self.offset.load(Ordering::Relaxed) as usize;
        offset
            .checked_add(channel_size)
            .and_then(|end| buffer.get(offset..end))
    }
    fn find_buffer_mut<'a>(
        &'a self,
        buffer: &'a mut [u8],
        channel_size: usize,
    ) -> Option<&'a mut [u8]> {
        let offset = self.offset.load(Ordering::Relaxed) as usize;
        offset
            .checked_add(channel_size)
            .and_then(|end| buffer.get_mut(offset..end))
    }
}

impl SharedMemoryChannel<HostToWorker> {
    fn read_ring(&self, buffer: &[u8], output: &mut [u8]) -> std::io::Result<usize> {
        if buffer.is_empty() || output.is_empty() {
            return Ok(0);
        }

        let write_pos = self.write.load(Ordering::Acquire) as usize;
        let read_pos = self.read.load(Ordering::Relaxed) as usize;
        let available = (write_pos.saturating_sub(read_pos)).min(buffer.len());
        let count = cmp::min(available, output.len());

        copy_from_ring(output, buffer, read_pos % buffer.len(), count);
        self.read
            .store((read_pos + count) as u64, Ordering::Release);

        Ok(count)
    }
}

impl SharedMemoryChannel<WorkerToHost> {
    fn write_ring(&self, buffer: &mut [u8], input: &[u8]) -> std::io::Result<usize> {
        if buffer.is_empty() || input.is_empty() {
            return Ok(0);
        }

        let write_pos = self.write.load(Ordering::Relaxed) as usize;
        let read_pos = self.read.load(Ordering::Acquire) as usize;
        let used = (write_pos.saturating_sub(read_pos)).min(buffer.len());
        let available = buffer.len() - used;
        let count = cmp::min(available, input.len());

        copy_to_ring(buffer, write_pos % buffer.len(), &input[..count]);
        self.write
            .store((write_pos + count) as u64, Ordering::Release);

        Ok(count)
    }
}

impl SharedMemoryInstance {
    const MAGIC: u32 = 0x95abe799;
    const VERSION: u32 = 1;

    fn verify(&self) -> bool {
        self.layout().magic == Self::MAGIC && self.layout().version == Self::VERSION
    }

    fn layout(&self) -> &SharedMemoryLayout {
        assert!(self.mmap.len() >= size_of::<SharedMemoryLayout>());
        let ptr = self.mmap.as_ptr();
        assert_eq!(ptr.align_offset(align_of::<SharedMemoryLayout>()), 0);
        unsafe { &*(ptr.cast::<SharedMemoryLayout>()) }
    }

    pub fn from_file(file: &File, size: usize) -> std::io::Result<Self> {
        assert!(size > size_of::<SharedMemoryLayout>());
        let mmap = open(file, size)?;
        let res = Self { mmap };
        if res.verify() {
            Ok(res)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Unsupported layout: magic = {:#08x}, version = {:#08x}",
                    res.layout().magic,
                    res.layout().version
                ),
            ))
        }
    }

    fn layout_payload_mut(&mut self) -> (&SharedMemoryLayout, &mut [u8]) {
        assert!(self.mmap.len() >= size_of::<SharedMemoryLayout>());
        let layout_size = size_of::<SharedMemoryLayout>();
        let (layout, payload) = self.mmap.split_at_mut(layout_size);
        let ptr = layout.as_ptr();
        assert_eq!(ptr.align_offset(align_of::<SharedMemoryLayout>()), 0);
        let layout = unsafe { ptr.cast::<SharedMemoryLayout>().as_ref_unchecked() };
        (layout, payload)
    }

    pub fn payload(&self) -> &[u8] {
        let layout_size = size_of::<SharedMemoryLayout>();
        let (_, payload) = self.mmap.split_at(layout_size);
        payload
    }
}

impl std::io::Read for SharedMemoryInstance {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let layout = self.layout();
        let buf = layout
            .host_to_worker
            .find_buffer(self.payload(), layout.channel_size as usize)
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid buffer")
            })?;
        layout.host_to_worker.read_ring(buf, output)
    }
}

impl std::io::Write for SharedMemoryInstance {
    fn write(&mut self, input: &[u8]) -> std::io::Result<usize> {
        let (layout, payload) = self.layout_payload_mut();
        let buf = layout
            .worker_to_host
            .find_buffer_mut(payload, layout.channel_size as usize)
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "Invalid buffer")
            })?;
        layout.worker_to_host.write_ring(buf, input)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn open(file: &File, size: usize) -> std::io::Result<MmapMut> {
    unsafe { MmapOptions::new().len(size).map_mut(file) }
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
