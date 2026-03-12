//! Video stream interface and implementation.

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
}

impl VideoStream {
    pub fn new() -> Self {
        VideoStream { next: None }
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

    fn update(&mut self) {}

    fn close(self: Box<Self>) {
        drop(self);
    }

    fn is_frame_ready(&self) -> Bool {
        true
    }

    fn frame_decompress(&mut self) {}

    fn frame_render(&mut self, _buffer: &mut dyn VideoBuffer) {}

    fn frame_next(&mut self) {}

    fn frame_index(&self) -> Int {
        0
    }

    fn frame_count(&self) -> Int {
        1
    }

    fn frame_goto(&mut self, _index: Int) {}

    fn height(&self) -> Int {
        0
    }

    fn width(&self) -> Int {
        0
    }
}
