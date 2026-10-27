//! GPU → CPU readback that works on native and on the web.
//!
//! `map_async` only *requests* a mapping; its callback runs when the device is
//! polled. Native code can block with `Device::poll(PollType::wait_indefinitely())`
//! ([`Readback::wait`]). On wasm32/WebGPU `poll` is a no-op and the browser fires
//! the callback from its event loop, so callers check [`Readback::status`] on later
//! frames instead of blocking.

use crate::{RenderError, Result};
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

const PENDING: u8 = 0;
const READY: u8 = 1;
const FAILED: u8 = 2;

/// State of an asynchronous readback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadbackStatus {
    /// The GPU has not finished yet.
    Pending,
    /// The data can be read.
    Ready,
    /// Mapping failed.
    Failed,
}

/// A `MAP_READ` buffer plus the flag its `map_async` callback sets.
#[derive(Debug)]
pub(crate) struct Readback {
    buffer: wgpu::Buffer,
    // An atomic instead of a Mutex: the callback must be `'static` (and `Send` on
    // native), it runs once, and nothing else needs to be guarded.
    state: Arc<AtomicU8>,
}

impl Readback {
    /// A buffer to copy `size` bytes into.
    pub(crate) fn new(device: &wgpu::Device, size: u64) -> Self {
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("kubit-render readback"),
            size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            buffer,
            state: Arc::new(AtomicU8::new(PENDING)),
        }
    }

    pub(crate) fn buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Requests the mapping. Call after the copy into the buffer was submitted: a
    /// buffer with a pending map cannot be used by a later submission.
    pub(crate) fn request_map(&self) {
        let state = Arc::clone(&self.state);
        self.buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let value = if result.is_ok() { READY } else { FAILED };
                state.store(value, Ordering::Release);
            });
    }

    pub(crate) fn status(&self) -> ReadbackStatus {
        match self.state.load(Ordering::Acquire) {
            READY => ReadbackStatus::Ready,
            FAILED => ReadbackStatus::Failed,
            _ => ReadbackStatus::Pending,
        }
    }

    /// Blocks until the mapping resolves (native only).
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn wait(&self, device: &wgpu::Device) -> Result<()> {
        device
            .poll(wgpu::PollType::wait_indefinitely())
            .map_err(|e| RenderError::Poll(e.to_string()))?;
        match self.status() {
            ReadbackStatus::Ready => Ok(()),
            _ => Err(RenderError::Readback),
        }
    }

    /// Copies the mapped bytes out and unmaps. Errors unless the status is `Ready`.
    pub(crate) fn bytes(&self) -> Result<Vec<u8>> {
        if self.status() != ReadbackStatus::Ready {
            return Err(RenderError::Readback);
        }
        let bytes = self
            .buffer
            .get_mapped_range(..)
            .map_err(|_| RenderError::Readback)?
            .to_vec();
        self.buffer.unmap();
        Ok(bytes)
    }
}
