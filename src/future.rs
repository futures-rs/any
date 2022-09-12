use std::{
    future::Future,
    pin::Pin,
    ptr::NonNull,
    task::{Context, Poll},
};

pub trait AnyFutureEx: Future {
    fn to_any_future(self) -> AnyFuture<Self::Output>
    where
        Self: Sized,
    {
        AnyFuture::new(self)
    }
}

impl<T: ?Sized> AnyFutureEx for T where T: Future {}

#[repr(C)]
struct AnyFutureVTable<Output> {
    poll_next:
        unsafe fn(ptr: NonNull<AnyFutureVTable<Output>>, cx: &mut Context<'_>) -> Poll<Output>,
}

impl<Output> AnyFutureVTable<Output> {
    fn new<Fut>() -> Self
    where
        Fut: Future<Output = Output>,
    {
        Self {
            poll_next: poll_next::<Fut, Output>,
        }
    }
}

unsafe fn poll_next<Fut, Output>(
    ptr: NonNull<AnyFutureVTable<Output>>,
    cx: &mut Context<'_>,
) -> Poll<Output>
where
    Fut: Future<Output = Output>,
{
    let mut fut = ptr.cast::<AnyFutureImpl<Fut, Output>>();

    let fut = fut.as_mut();

    fut.inner.as_mut().poll(cx)
}

#[repr(C)]
struct AnyFutureImpl<Fut, Output> {
    vtable: AnyFutureVTable<Output>,
    inner: Pin<Box<Fut>>,
}

pub struct AnyFuture<Output> {
    ptr: NonNull<AnyFutureVTable<Output>>,
}

unsafe impl<Output> Send for AnyFuture<Output> {}

unsafe impl<Output> Sync for AnyFuture<Output> {}

impl<Output> AnyFuture<Output> {
    fn new<Fut>(inner: Fut) -> Self
    where
        Fut: Future<Output = Output>,
    {
        let boxed = Box::new(AnyFutureImpl::<Fut, Output> {
            vtable: AnyFutureVTable::new::<Fut>(),
            inner: Box::pin(inner),
        });

        let ptr =
            unsafe { NonNull::new_unchecked(Box::into_raw(boxed) as *mut AnyFutureVTable<Output>) };

        AnyFuture { ptr }
    }
}

impl<Output> Future for AnyFuture<Output> {
    type Output = Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        unsafe {
            let poll_next = self.ptr.as_ref().poll_next;

            poll_next(self.ptr, cx)
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[async_std::test]
    async fn test_any_future() -> Result<(), anyhow::Error> {
        let fut = futures::future::poll_immediate(async { 1 });

        let fut = fut.to_any_future();

        let handle = async_std::task::spawn(async move {
            assert_eq!(fut.await, Some(1));
        });

        handle.await;

        Ok(())
    }
}
