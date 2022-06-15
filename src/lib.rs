mod sink;
mod stream;

mod raw;

use std::{
    pin::Pin,
    ptr::NonNull,
    task::{Context, Poll},
};

use raw::{FramedHeader, RawChannel};

use futures::{Sink, Stream};

pub use sink::*;

pub use stream::*;

pub trait ErasureStream: Stream {
    fn erasure_stream(&mut self) -> ErasureRead<Self::Item>
    where
        Self: Unpin,
    {
        ErasureRead::new(self)
    }
}

pub trait ErasureSink<Item, Error>: Sink<Item, Error = Error> {
    fn erasure_sink(&mut self) -> ErasureWrite<Item, Error>
    where
        Self: Unpin,
    {
        ErasureWrite::new(self)
    }
}

/// Stream + Sink type erasure struct
///
pub struct Erasure<Output, Input, Error> {
    raw: NonNull<FramedHeader<Output, Input, Error>>,
}

impl<Output, Input, Error> Erasure<Output, Input, Error> {
    pub fn new<T>(inner: T) -> Self
    where
        T: Stream<Item = Output> + Sink<Input, Error = Error>,
    {
        Erasure {
            raw: RawChannel::new_framed(inner),
        }
    }
}

impl<Output, Input, Error> Stream for Erasure<Output, Input, Error> {
    type Item = Output;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let raw = unsafe { self.raw.as_ref() };

        let poll_next = raw.stream_vtable.as_ref().unwrap().poll_next;

        unsafe { poll_next(self.raw, cx) }
    }
}

impl<Output, Input, Error> Sink<Input> for Erasure<Output, Input, Error> {
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
