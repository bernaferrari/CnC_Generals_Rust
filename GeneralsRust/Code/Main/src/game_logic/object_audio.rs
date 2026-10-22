//! Ordered object audio at the driving operation's observation point.
//!
//! This loan is never stored on an Object, cloned, serialized or published.
use super::{AudioEventRequest, Object};

enum Sink<'a> {
    Queue(&'a mut Vec<AudioEventRequest>),
    Callback(&'a mut dyn FnMut(&mut Object, AudioEventRequest)),
}

pub struct ObjectAudio<'a> {
    sink: Sink<'a>,
}

impl<'a> ObjectAudio<'a> {
    pub fn queue(output: &'a mut Vec<AudioEventRequest>) -> Self {
        Self {
            sink: Sink::Queue(output),
        }
    }

    pub(in crate::game_logic) fn callback(
        emit: &'a mut dyn FnMut(&mut Object, AudioEventRequest),
    ) -> Self {
        Self {
            sink: Sink::Callback(emit),
        }
    }

    /// Take the live handle once, before movement exit cleanup can move it.
    pub fn stop_loop(&mut self, object: &mut Object) {
        let Some(name) = object.move_loop_audio.take() else {
            return;
        };
        let request = AudioEventRequest::new(&name)
            .with_object(object.id)
            .with_position(object.get_position())
            .stopping();
        self.submit(object, request);
    }

    /// Deliver at the current synchronous Object/Body observation point.
    pub fn submit(&mut self, object: &mut Object, request: AudioEventRequest) {
        match &mut self.sink {
            Sink::Queue(output) => output.push(request),
            Sink::Callback(emit) => emit(object, request),
        }
    }
}
