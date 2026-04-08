//! Video stream interface and implementation.

use std::time::{Duration, Instant};

use crate::video_buffer::VideoBuffer;

type Bool = bool;
type Int = i32;

pub trait VideoStreamInterface: Send {
    fn next(&self) -> Option<&dyn VideoStreamInterface>;
    fn next_mut(&mut self) -> Option<&mut dyn VideoStreamInterface>;
    fn update(&mut self);
    fn close(self: Box<Self>);
    fn is_frame_ready(&self) -> Bool;
    fn frame_decompress(&mut self);
    fn frame_render(&mut self, buffer: &mut dyn VideoBuffer);
    fn frame_next(&mut self);
    fn frame_index(&self) -> Int;
    fn frame_count(&self) -> Int;
    fn frame_goto(&mut self, index: Int);
    fn height(&self) -> Int;
    fn width(&self) -> Int;
}

pub struct VideoStream {
    next: Option<Box<VideoStream>>,
    current_frame: Int,
    total_frames: Int,
    width: Int,
    height: Int,
    frame_duration: Duration,
    last_update: Instant,
    paused: Bool,
    decoded_rgba: Vec<u8>,
}

impl VideoStream {
    pub fn new() -> Self {
        VideoStream {
            next: None,
            current_frame: 0,
            total_frames: 1,
            width: 0,
            height: 0,
            frame_duration: Duration::from_millis(33),
            last_update: Instant::now(),
            paused: false,
            decoded_rgba: Vec::new(),
        }
    }

    pub fn get_next(&self) -> Option<&VideoStream> {
        self.next.as_deref()
    }

    pub fn get_next_mut(&mut self) -> Option<&mut VideoStream> {
        self.next.as_deref_mut()
    }

    pub fn set_next(&mut self, next: Option<Box<VideoStream>>) {
        self.next = next;
    }

    pub fn take_next(&mut self) -> Option<Box<VideoStream>> {
        self.next.take()
    }
}

impl Default for VideoStream {
    fn default() -> Self {
        Self::new()
    }
}

impl VideoStreamInterface for VideoStream {
    fn next(&self) -> Option<&dyn VideoStreamInterface> {
        self.next.as_deref().map(|s| s as &dyn VideoStreamInterface)
    }

    fn next_mut(&mut self) -> Option<&mut dyn VideoStreamInterface> {
        self.next
            .as_deref_mut()
            .map(|s| s as &mut dyn VideoStreamInterface)
    }

    fn update(&mut self) {
        if self.paused || self.total_frames <= 1 {
            self.last_update = Instant::now();
            return;
        }

        let now = Instant::now();
        let elapsed = now.saturating_duration_since(self.last_update);
        if elapsed < self.frame_duration {
            return;
        }

        let frames_to_advance = (elapsed.as_nanos() / self.frame_duration.as_nanos().max(1)) as i32;
        self.last_update = now;
        for _ in 0..frames_to_advance.max(1) {
            self.frame_next();
        }
    }

    fn close(self: Box<Self>) {
        drop(self);
    }

    fn is_frame_ready(&self) -> Bool {
        true
    }

    fn frame_decompress(&mut self) {
        let width = self.width.max(1) as usize;
        let height = self.height.max(1) as usize;
        self.decoded_rgba.resize(width * height * 4, 0);

        let phase = (self.current_frame.rem_euclid(255)) as u8;
        for y in 0..height {
            for x in 0..width {
                let idx = (y * width + x) * 4;
                self.decoded_rgba[idx] = phase;
                self.decoded_rgba[idx + 1] = ((x * 255) / width.max(1)) as u8;
                self.decoded_rgba[idx + 2] = ((y * 255) / height.max(1)) as u8;
                self.decoded_rgba[idx + 3] = 0xFF;
            }
        }
    }

    fn frame_render(&mut self, buffer: &mut dyn VideoBuffer) {
        if !buffer.valid() {
            return;
        }
        if self.decoded_rgba.is_empty() {
            self.frame_decompress();
        }

        let copy_width = self.width.max(0) as usize;
        let copy_height = self.height.max(0) as usize;
        let pitch = buffer.pitch() as usize;
        let bytes_per_pixel = match buffer.format() {
            crate::video_buffer::VideoBufferType::R8G8B8 => 3,
            crate::video_buffer::VideoBufferType::X8R8G8B8 => 4,
            crate::video_buffer::VideoBufferType::R5G6B5
            | crate::video_buffer::VideoBufferType::X1R5G5B5 => 2,
            crate::video_buffer::VideoBufferType::Unknown => 0,
        };
        if bytes_per_pixel == 0 {
            return;
        }

        let dst = buffer.lock();
        if dst.is_null() {
            return;
        }

        let max_width = copy_width.min(buffer.width() as usize);
        let max_height = copy_height.min(buffer.height() as usize);
        for y in 0..max_height {
            for x in 0..max_width {
                let src_index = (y * copy_width + x) * 4;
                let rgba = &self.decoded_rgba[src_index..src_index + 4];
                let dst_index = y * pitch + x * bytes_per_pixel;
                unsafe {
                    match buffer.format() {
                        crate::video_buffer::VideoBufferType::R8G8B8 => {
                            *dst.add(dst_index) = rgba[0];
                            *dst.add(dst_index + 1) = rgba[1];
                            *dst.add(dst_index + 2) = rgba[2];
                        }
                        crate::video_buffer::VideoBufferType::X8R8G8B8 => {
                            *dst.add(dst_index) = rgba[2];
                            *dst.add(dst_index + 1) = rgba[1];
                            *dst.add(dst_index + 2) = rgba[0];
                            *dst.add(dst_index + 3) = 0xFF;
                        }
                        crate::video_buffer::VideoBufferType::R5G6B5 => {
                            let packed = (((rgba[0] as u16 >> 3) & 0x1F) << 11)
                                | (((rgba[1] as u16 >> 2) & 0x3F) << 5)
                                | ((rgba[2] as u16 >> 3) & 0x1F);
                            *dst.add(dst_index) = (packed & 0xFF) as u8;
                            *dst.add(dst_index + 1) = (packed >> 8) as u8;
                        }
                        crate::video_buffer::VideoBufferType::X1R5G5B5 => {
                            let packed = 0x8000
                                | (((rgba[0] as u16 >> 3) & 0x1F) << 10)
                                | (((rgba[1] as u16 >> 3) & 0x1F) << 5)
                                | ((rgba[2] as u16 >> 3) & 0x1F);
                            *dst.add(dst_index) = (packed & 0xFF) as u8;
                            *dst.add(dst_index + 1) = (packed >> 8) as u8;
                        }
                        crate::video_buffer::VideoBufferType::Unknown => {}
                    }
                }
            }
        }

        buffer.unlock();
    }

    fn frame_next(&mut self) {
        if self.total_frames <= 0 {
            self.current_frame = 0;
        } else {
            self.current_frame = (self.current_frame + 1).rem_euclid(self.total_frames.max(1));
        }
        self.frame_decompress();
    }

    fn frame_index(&self) -> Int {
        self.current_frame
    }

    fn frame_count(&self) -> Int {
        self.total_frames
    }

    fn frame_goto(&mut self, index: Int) {
        if self.total_frames <= 0 {
            self.current_frame = 0;
        } else {
            self.current_frame = index.clamp(0, self.total_frames - 1);
        }
        self.frame_decompress();
    }

    fn height(&self) -> Int {
        self.height
    }

    fn width(&self) -> Int {
        self.width
    }
}
