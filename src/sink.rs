use std::{
    pin::Pin,
    ptr::NonNull,
    task::{Context, Poll},
};

use super::raw::{FramedHeader, RawChannel};

use futures::Sink;

pub struct ErasureWrite<Input, Error> {
    raw: NonNull<FramedHeader<(), Input, Error>>,
}

impl<Input, Error> ErasureWrite<Input, Error> {
    pub fn new<T>(inner: T) -> Self
    where
        T: Sink<Input, Error = Error>,
    {
        ErasureWrite {
            raw: RawChannel::new_write(inner),
        }
    }
}

impl<Input, Error> Sink<Input> for ErasureWrite<Input, Error> {
    type Error = Error;

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let raw = unsafe { self.raw.as_ref() };

        let poll_ready = raw.sink_vtable.as_ref().unwrap().poll_ready;

        unsafe { poll_ready(self.raw, cx) }
    }

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let raw = unsafe { self.raw.as_ref() };

        let poll_close = raw.sink_vtable.as_ref().unwrap().poll_close;

        unsafe { poll_close(self.raw, cx) }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        let raw = unsafe { self.raw.as_ref() };

        let poll_flush = raw.sink_vtable.as_ref().unwrap().poll_flush;

        unsafe { poll_flush(self.raw, cx) }
    }

    fn start_send(self: Pin<&mut Self>, item: Input) -> Result<(), Self::Error> {
        let raw = unsafe { self.raw.as_ref() };

        let start_send = raw.sink_vtable.as_ref().unwrap().start_send;

        unsafe { start_send(self.raw, item) }
    }
}
