use futures::{Sink, Stream, StreamExt};

use std::{
    marker::PhantomData,
    ptr::NonNull,
    sync::Mutex,
    task::{Context, Poll},
};

use std::pin::Pin;

#[repr(C)]
pub struct StreamVTable<Output, Input, Error> {
    poll_next: unsafe fn(
        NonNull<StreamSinkVTable<Output, Input, Error>>,
        &mut Context<'_>,
    ) -> Poll<Option<Input>>,

    drop: unsafe fn(NonNull<StreamSinkVTable<Output, Input, Error>>),
}

impl<Output, Input, Error> StreamVTable<Output, Input, Error> {
    fn new<S>() -> Self
    where
        S: Stream<Item = Input> + Unpin,
    {
        StreamVTable {
            drop: drop::<S, Output, Input, Error>,
            poll_next: poll_next::<S, Output, Input, Error>,
        }
    }
}

#[repr(C)]
pub struct SinkVTable<Output, Input, Error> {
    drop: unsafe fn(NonNull<StreamSinkVTable<Output, Input, Error>>),
    pub poll_ready: unsafe fn(
        NonNull<StreamSinkVTable<Output, Input, Error>>,
        &mut Context<'_>,
    ) -> Poll<Result<(), Error>>,

    pub start_send: unsafe fn(
        NonNull<StreamSinkVTable<Output, Input, Error>>,
        item: Output,
    ) -> Result<(), Error>,

    pub poll_flush: unsafe fn(
        NonNull<StreamSinkVTable<Output, Input, Error>>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Error>>,

    pub poll_close: unsafe fn(
        NonNull<StreamSinkVTable<Output, Input, Error>>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<(), Error>>,

    _maker: PhantomData<Output>,
}

impl<Output, Input, Error> SinkVTable<Output, Input, Error> {
    fn new<T>() -> Self
    where
        T: Sink<Output, Error = Error> + Unpin,
    {
        SinkVTable {
            drop: drop::<T, Output, Input, Error>,
            poll_ready: poll_ready::<T, Output, Input, Error>,
            start_send: start_send::<T, Output, Input, Error>,
            poll_flush: poll_flush::<T, Output, Input, Error>,
            poll_close: poll_close::<T, Output, Input, Error>,
            _maker: PhantomData,
        }
    }
}

unsafe fn drop<S, Output, Input, Error>(vtable: NonNull<StreamSinkVTable<Output, Input, Error>>) {
    let raw = vtable.cast::<RawStreamSink<S, Output, Input, Error>>();

    Box::from_raw(raw.as_ptr());
}

unsafe fn poll_next<S, Output, Input, Error>(
    vtable: NonNull<StreamSinkVTable<Output, Input, Error>>,
    cx: &mut Context<'_>,
) -> Poll<Option<Input>>
where
    S: Stream<Item = Input> + Unpin,
{
    let mut stream = vtable.cast::<RawStreamSink<S, Output, Input, Error>>();

    stream.as_mut().inner.poll_next_unpin(cx)
}

unsafe fn poll_ready<T, Output, Input, Error>(
    ptr: NonNull<StreamSinkVTable<Output, Input, Error>>,
    cx: &mut Context<'_>,
) -> Poll<Result<(), Error>>
where
    T: Sink<Output, Error = Error>,
{
    let mut channel = ptr.cast::<RawStreamSink<T, Output, Input, Error>>();

    let channel = channel.as_mut();

    channel.inner.as_mut().poll_ready(cx)
}

unsafe fn start_send<T, Output, Input, Error>(
    ptr: NonNull<StreamSinkVTable<Output, Input, Error>>,
    item: Output,
) -> Result<(), Error>
where
    T: Sink<Output, Error = Error>,
{
    let mut channel = ptr.cast::<RawStreamSink<T, Output, Input, Error>>();

    let channel = channel.as_mut();

    channel.inner.as_mut().start_send(item)
}

unsafe fn poll_flush<T, Output, Input, Error>(
    ptr: NonNull<StreamSinkVTable<Output, Input, Error>>,
    cx: &mut Context<'_>,
) -> Poll<Result<(), Error>>
where
    T: Sink<Output, Error = Error>,
{
    let mut channel = ptr.cast::<RawStreamSink<T, Output, Input, Error>>();

    let channel = channel.as_mut();

    channel.inner.as_mut().poll_flush(cx)
}

unsafe fn poll_close<T, Output, Input, Error>(
    ptr: NonNull<StreamSinkVTable<Output, Input, Error>>,
    cx: &mut Context<'_>,
) -> Poll<Result<(), Error>>
where
    T: Sink<Output, Error = Error>,
{
    let mut channel = ptr.cast::<RawStreamSink<T, Output, Input, Error>>();

    let channel = channel.as_mut();

    channel.inner.as_mut().poll_close(cx)
}

#[repr(C)]
pub struct StreamSinkVTable<Output, Input, Error> {
    stream: Option<StreamVTable<Output, Input, Error>>,
    sink: Option<SinkVTable<Output, Input, Error>>,
}

#[repr(C)]
pub struct RawStreamSink<S, Output, Input, Error> {
    vtable: StreamSinkVTable<Output, Input, Error>,
    inner: Pin<Box<S>>,
}

impl<S, Output, Input, Error> RawStreamSink<S, Output, Input, Error> {
    pub fn new(inner: S) -> NonNull<StreamSinkVTable<Output, Input, Error>>
    where
        S: Sink<Output> + Stream<Item = Input> + Unpin,
    {
        let boxed = Box::new(RawStreamSink {
            vtable: StreamSinkVTable::<(), Input, ()> {
                stream: Some(StreamVTable::new::<S>()),
                sink: None,
            },
            inner: Box::pin(inner),
        });

        unsafe {
            NonNull::new_unchecked(
                Box::into_raw(boxed) as *mut StreamSinkVTable<Output, Input, Error>
            )
        }
    }
}

impl<S, Input> RawStreamSink<S, (), Input, ()> {
    pub fn new_stream(inner: S) -> NonNull<StreamSinkVTable<(), Input, ()>>
    where
        S: Stream<Item = Input> + Unpin,
    {
        let boxed = Box::new(RawStreamSink {
            vtable: StreamSinkVTable::<(), Input, ()> {
                stream: Some(StreamVTable::new::<S>()),
                sink: None,
            },
            inner: Box::pin(inner),
        });

        unsafe {
            NonNull::new_unchecked(Box::into_raw(boxed) as *mut StreamSinkVTable<(), Input, ()>)
        }
    }
}

impl<S, Output, Error> RawStreamSink<S, Output, (), Error> {
    pub fn new_sink(inner: S) -> NonNull<StreamSinkVTable<Output, (), Error>>
    where
        S: Sink<Output, Error = Error> + Unpin,
    {
        let boxed = Box::new(RawStreamSink {
            vtable: StreamSinkVTable::<Output, (), Error> {
                sink: Some(SinkVTable::new::<S>()),
                stream: None,
            },
            inner: Box::pin(inner),
        });

        unsafe {
            NonNull::new_unchecked(Box::into_raw(boxed) as *mut StreamSinkVTable<Output, (), Error>)
        }
    }
}

struct StreamSink<Output, Input, Error>(NonNull<StreamSinkVTable<Output, Input, Error>>);

impl<Output, Input, Error> From<NonNull<StreamSinkVTable<Output, Input, Error>>>
    for StreamSink<Output, Input, Error>
{
    fn from(ptr: NonNull<StreamSinkVTable<Output, Input, Error>>) -> Self {
        StreamSink(ptr)
    }
}

unsafe impl<Output, Input, Error> Send for StreamSink<Output, Input, Error> {}

pub struct AnyStream<Item> {
    vtable: Mutex<StreamSink<(), Item, ()>>,
}

impl<Item> AnyStream<Item> {
    pub fn new<S>(inner: S) -> Self
    where
        S: Stream<Item = Item> + Unpin,
    {
        AnyStream {
            vtable: Mutex::new(RawStreamSink::new_stream(inner).into()),
        }
    }
}

impl<Item> Drop for AnyStream<Item> {
    fn drop(&mut self) {
        let vtable = self.vtable.lock().unwrap();
        unsafe {
            let drop = vtable.0.as_ref().stream.as_ref().unwrap().drop;

            drop(vtable.0);
        }
    }
}

impl<Item> Stream for AnyStream<Item> {
    type Item = Item;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let vtable = self.vtable.lock().unwrap();
        unsafe {
            let poll_next = vtable.0.as_ref().stream.as_ref().unwrap().poll_next;

            poll_next(vtable.0, cx)
        }
    }
}

/// Cast any type stream to AnyStream
pub trait AnyStreamEx: Stream {
    fn to_any_stream(self) -> AnyStream<Self::Item>
    where
        Self: Unpin + Sized,
    {
        AnyStream::new(self)
    }
}

impl<T: ?Sized> AnyStreamEx for T where T: Stream {}

pub struct AnySink<Item, Error> {
    vtable: Mutex<StreamSink<Item, (), Error>>,
}

impl<Item, Error> AnySink<Item, Error> {
    pub fn new<S>(inner: S) -> Self
    where
        S: Sink<Item, Error = Error> + Unpin,
    {
        AnySink {
            vtable: Mutex::new(RawStreamSink::new_sink(inner).into()),
        }
    }
}

impl<Item, Error> Drop for AnySink<Item, Error> {
    fn drop(&mut self) {
        let vtable = self.vtable.lock().unwrap();
        unsafe {
            let drop = vtable.0.as_ref().sink.as_ref().unwrap().drop;

            drop(vtable.0);
        }
    }
}

impl<Item, Error> Sink<Item> for AnySink<Item, Error> {
    type Error = Error;

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        log::trace!("poll_close");
        let vtable = self.vtable.lock().unwrap();

        unsafe {
            let poll_close = vtable.0.as_ref().sink.as_ref().unwrap().poll_close;

            poll_close(vtable.0, cx)
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        log::trace!("poll_flush");
        let vtable = self.vtable.lock().unwrap();
        unsafe {
            let poll_flush = vtable.0.as_ref().sink.as_ref().unwrap().poll_flush;

            poll_flush(vtable.0, cx)
        }
    }

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        log::trace!("poll_ready");
        let vtable = self.vtable.lock().unwrap();
        unsafe {
            let vtable = vtable.0;

            let poll_ready = vtable.as_ref().sink.as_ref().unwrap().poll_ready;

            poll_ready(vtable, cx)
        }
    }

    fn start_send(self: Pin<&mut Self>, item: Item) -> Result<(), Self::Error> {
        log::trace!("start_send");
        let vtable = self.vtable.lock().unwrap();
        unsafe {
            let start_send = vtable.0.as_ref().sink.as_ref().unwrap().start_send;

            start_send(vtable.0, item)
        }
    }
}

/// Cast any type stream to AnyStream
pub trait AnySinkEx<Item>: Sink<Item> {
    fn to_any_sink(self) -> AnySink<Item, Self::Error>
    where
        Self: Unpin + Sized,
    {
        AnySink::new(self)
    }
}

impl<T: ?Sized, Item> AnySinkEx<Item> for T where T: Sink<Item> {}

pub struct AnyStreamSink<Output, Input, Error> {
    vtable: Mutex<StreamSink<Output, Input, Error>>,
}

impl<Output, Input, Error> AnyStreamSink<Output, Input, Error> {
    pub fn new<S>(inner: S) -> Self
    where
        S: Stream<Item = Input> + Sink<Output, Error = Error> + Unpin,
    {
        AnyStreamSink {
            vtable: Mutex::new(RawStreamSink::new(inner).into()),
        }
    }
}

impl<Output, Input, Error> Drop for AnyStreamSink<Output, Input, Error> {
    fn drop(&mut self) {
        let vtable = self.vtable.lock().unwrap();
        unsafe {
            let drop = vtable.0.as_ref().sink.as_ref().unwrap().drop;

            drop(vtable.0);
        }
    }
}

impl<Output, Input, Error> Sink<Output> for AnyStreamSink<Output, Input, Error> {
    type Error = Error;

    fn poll_close(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        log::trace!("poll_close");
        let vtable = self.vtable.lock().unwrap();

        unsafe {
            let poll_close = vtable.0.as_ref().sink.as_ref().unwrap().poll_close;

            poll_close(vtable.0, cx)
        }
    }

    fn poll_flush(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        log::trace!("poll_flush");
        let vtable = self.vtable.lock().unwrap();
        unsafe {
            let poll_flush = vtable.0.as_ref().sink.as_ref().unwrap().poll_flush;

            poll_flush(vtable.0, cx)
        }
    }

    fn poll_ready(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        log::trace!("poll_ready");
        let vtable = self.vtable.lock().unwrap();
        unsafe {
            let vtable = vtable.0;

            let poll_ready = vtable.as_ref().sink.as_ref().unwrap().poll_ready;

            poll_ready(vtable, cx)
        }
    }

    fn start_send(self: Pin<&mut Self>, item: Output) -> Result<(), Self::Error> {
        log::trace!("start_send");
        let vtable = self.vtable.lock().unwrap();
        unsafe {
            let start_send = vtable.0.as_ref().sink.as_ref().unwrap().start_send;

            start_send(vtable.0, item)
        }
    }
}

impl<Output, Input, Error> Stream for AnyStreamSink<Output, Input, Error> {
    type Item = Input;

    fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let vtable = self.vtable.lock().unwrap();
        unsafe {
            let poll_next = vtable.0.as_ref().stream.as_ref().unwrap().poll_next;

            poll_next(vtable.0, cx)
        }
    }
}

#[cfg(test)]
mod tests {

    use std::task::Poll;

    use futures::SinkExt;

    use super::*;

    #[async_std::test]
    async fn test_anystream() -> Result<(), anyhow::Error> {
        let stream = futures::stream::poll_fn(|_| Poll::Ready(Some("Hello".to_owned())));

        async_std::task::spawn(async move {
            stream.to_any_stream().next().await;
        });

        let mut sink = futures::sink::drain::<String>().to_any_sink();

        async_std::task::spawn(async move {
            sink.send("hello".to_owned()).await?;

            Ok::<(), anyhow::Error>(())
        });

        Ok(())
    }
}
