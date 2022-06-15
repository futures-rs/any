use std::ptr::NonNull;

use super::raw::{FramedHeader, RawChannel};

use futures::Stream;

pub struct ErasureRead<Output> {
    raw: NonNull<FramedHeader<Output, (), ()>>,
}

impl<Output> ErasureRead<Output> {
    pub fn new<T>(inner: T) -> Self
    where
        T: Stream<Item = Output>,
    {
        ErasureRead {
            raw: RawChannel::new_read(inner),
        }
    }
}

impl<Output> Stream for ErasureRead<Output> {
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
