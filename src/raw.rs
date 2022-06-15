use std::{
    marker::PhantomData,
    pin::Pin,
    ptr::NonNull,
    task::{Context, Poll},
};

use futures::{Sink, Stream};

pub struct StreamVtable<Output, Input, Error> {
    pub poll_next: unsafe fn(
        NonNull<FramedHeader<Output, Input, Error>>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Output>>,

    _input: PhantomData<Input>,

    _err: PhantomData<Error>,
}

pub struct SinkVtable<Output, Input, Error> {
    pub poll_ready: unsafe fn(
        NonNull<FramedHeader<Output, Input, Error>>,
        &mut Context<'_>,
    ) -> Poll<Result<(), Error>>,

    pub start_send:
        unsafe fn(NonNull<FramedHeader<Output, Input, Error>>, item: Input) -> Result<(), Error>,

    pub poll_flush: unsafe fn(
        NonNull<FramedHeader<Output, Input, Error>>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Error>>,

    pub poll_close: unsafe fn(
        NonNull<FramedHeader<Output, Input, Error>>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Error>>,

    _output: PhantomData<Output>,
}

impl<Output, Input, Error> StreamVtable<Output, Input, Error> {
    fn new<T>() -> Self
    where
        T: Stream<Item = Output>,
    {
        StreamVtable {
            poll_next: poll_next::<T, Output, Input, Error>,
            _input: PhantomData,
            _err: PhantomData,
        }
    }
}

impl<Output, Input, Error> SinkVtable<Output, Input, Error> {
    fn new<T>() -> Self
    where
        T: Sink<Input, Error = Error>,
    {
        SinkVtable {
            poll_ready: poll_ready::<T, Output, Input, Error>,
            start_send: start_send::<T, Output, Input, Error>,
            poll_flush: poll_flush::<T, Output, Input, Error>,
            poll_close: poll_close::<T, Output, Input, Error>,
            _output: PhantomData,
        }
    }
}

unsafe fn poll_ready<T, Output, Input, Error>(
    ptr: NonNull<FramedHeader<Output, Input, Error>>,
    cx: &mut Context<'_>,
) -> Poll<Result<(), Error>>
where
    T: Sink<Input, Error = Error>,
{
    let mut channel = ptr.cast::<RawChannel<T, Output, Input, Error>>();

    let channel = channel.as_mut();

    channel.inner.as_mut().poll_ready(cx)
}

unsafe fn start_send<T, Output, Input, Error>(
    ptr: NonNull<FramedHeader<Output, Input, Error>>,
    item: Input,
) -> Result<(), Error>
where
    T: Sink<Input, Error = Error>,
{
    let mut channel = ptr.cast::<RawChannel<T, Output, Input, Error>>();

    let channel = channel.as_mut();

    channel.inner.as_mut().start_send(item)
}

unsafe fn poll_flush<T, Output, Input, Error>(
    ptr: NonNull<FramedHeader<Output, Input, Error>>,
    cx: &mut Context<'_>,
) -> Poll<Result<(), Error>>
where
    T: Sink<Input, Error = Error>,
{
    let mut channel = ptr.cast::<RawChannel<T, Output, Input, Error>>();

    let channel = channel.as_mut();

    channel.inner.as_mut().poll_flush(cx)
}

unsafe fn poll_close<T, Output, Input, Error>(
    ptr: NonNull<FramedHeader<Output, Input, Error>>,
    cx: &mut Context<'_>,
) -> Poll<Result<(), Error>>
where
    T: Sink<Input, Error = Error>,
{
    let mut channel = ptr.cast::<RawChannel<T, Output, Input, Error>>();

    let channel = channel.as_mut();

    channel.inner.as_mut().poll_close(cx)
}

unsafe fn poll_next<T, Output, Input, Error>(
    ptr: NonNull<FramedHeader<Output, Input, Error>>,
    cx: &mut Context<'_>,
) -> Poll<Option<Output>>
where
    T: Stream<Item = Output>,
{
    let mut channel = ptr.cast::<RawChannel<T, Output, Input, Error>>();

    let channel = channel.as_mut();

    channel.inner.as_mut().poll_next(cx)
}

#[repr(C)]
pub struct FramedHeader<Output, Input, Error> {
    pub(crate) stream_vtable: Option<StreamVtable<Output, Input, Error>>,
    pub(crate) sink_vtable: Option<SinkVtable<Output, Input, Error>>,
}

#[repr(C)]
pub struct RawChannel<T, Output, Input, Error> {
    header: FramedHeader<Output, Input, Error>,
    inner: Pin<Box<T>>,
}

impl<T, Output, Input, Error> RawChannel<T, Output, Input, Error> {
    pub fn new_read(inner: T) -> NonNull<FramedHeader<Output, Input, Error>>
    where
        T: Stream<Item = Output>,
    {
        let boxed = Box::new(RawChannel::<T, Output, Input, Error> {
            header: FramedHeader {
                stream_vtable: Some(StreamVtable::new::<T>()),
                sink_vtable: None,
            },
            inner: Box::pin(inner),
        });

        unsafe {
            NonNull::new_unchecked(Box::into_raw(boxed) as *mut FramedHeader<Output, Input, Error>)
        }
    }

    pub fn new_write(inner: T) -> NonNull<FramedHeader<Output, Input, Error>>
    where
        T: Sink<Input, Error = Error>,
    {
        let boxed = Box::new(RawChannel::<T, Output, Input, Error> {
            header: FramedHeader {
                stream_vtable: None,
                sink_vtable: Some(SinkVtable::new::<T>()),
            },
            inner: Box::pin(inner),
        });

        unsafe {
            NonNull::new_unchecked(Box::into_raw(boxed) as *mut FramedHeader<Output, Input, Error>)
        }
    }

    pub fn new_framed(inner: T) -> NonNull<FramedHeader<Output, Input, Error>>
    where
        T: Stream<Item = Output> + Sink<Input, Error = Error>,
    {
        let boxed = Box::new(RawChannel::<T, Output, Input, Error> {
            header: FramedHeader {
                stream_vtable: Some(StreamVtable::new::<T>()),
                sink_vtable: Some(SinkVtable::new::<T>()),
            },
            inner: Box::pin(inner),
        });

        unsafe {
            NonNull::new_unchecked(Box::into_raw(boxed) as *mut FramedHeader<Output, Input, Error>)
        }
    }
}
