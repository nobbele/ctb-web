use slotmap::SlotMap;
use std::{any::Any, cell::Cell, future::Future, marker::PhantomData, pin::Pin};

fn null_waker() -> std::task::Waker {
    let w = ();
    fn _nothing1(_: *const ()) -> std::task::RawWaker {
        //panic!()
        std::task::RawWaker::new(
            &() as *const (),
            &std::task::RawWakerVTable::new(_nothing1, _nothing2, _nothing3, _nothing4),
        )
    }
    fn _nothing2(_: *const ()) {
        //panic!()
    }
    fn _nothing3(_: *const ()) {
        //panic!()
    }
    fn _nothing4(_: *const ()) {}
    unsafe {
        std::task::Waker::from_raw(std::task::RawWaker::new(
            &w as *const (),
            &std::task::RawWakerVTable::new(_nothing1, _nothing2, _nothing3, _nothing4),
        ))
    }
}

type Fut = dyn Future<Output = Box<dyn Any>>;
enum FutureOrValue {
    Future(Pin<Box<Fut>>),
    Value(Box<dyn Any>),
}

pub struct PromiseExecutor {
    promises: SlotMap<slotmap::DefaultKey, FutureOrValue>,
}

impl PromiseExecutor {
    pub fn new() -> Self {
        PromiseExecutor {
            promises: SlotMap::new(),
        }
    }

    pub fn spawn<T: Send + 'static, F>(&mut self, fut: F) -> Promise<T>
    where
        F: Future<Output = T> + 'static,
    {
        let key = self.promises.insert(FutureOrValue::Future(Box::pin(async {
            Box::new(fut.await) as Box<dyn Any>
        })));
        Promise {
            cancelled_or_finished: Cell::new(true),
            id: key,
            _phantom: PhantomData,
        }
    }

    pub fn try_get<T: 'static>(&mut self, promise: &Promise<T>) -> Option<T> {
        match &self.promises[promise.id] {
            FutureOrValue::Future(_) => None,
            FutureOrValue::Value(_) => {
                promise.cancelled_or_finished.set(true);
                match self.promises.remove(promise.id).unwrap() {
                    FutureOrValue::Future(_) => unreachable!(),
                    FutureOrValue::Value(v) => Some(*v.downcast().unwrap()),
                }
            }
        }
    }

    pub fn poll(&mut self) {
        let keys = self.promises.keys().collect::<Vec<_>>();
        for key in keys {
            let promise = &mut self.promises[key];
            match promise {
                FutureOrValue::Future(f) => {
                    let waker = null_waker();
                    let mut cx = std::task::Context::from_waker(&waker);
                    match std::future::Future::poll(f.as_mut(), &mut cx) {
                        std::task::Poll::Ready(v) => {
                            self.promises[key] = FutureOrValue::Value(v);
                        }
                        std::task::Poll::Pending => {}
                    }
                }
                FutureOrValue::Value(_) => {}
            }
        }
    }

    pub fn cancel<T>(&mut self, promise: &Promise<T>) {
        promise.cancelled_or_finished.set(true);
        self.promises.remove(promise.id);
    }
}

impl Default for PromiseExecutor {
    fn default() -> Self {
        PromiseExecutor::new()
    }
}

pub struct Promise<T> {
    cancelled_or_finished: Cell<bool>,
    id: slotmap::DefaultKey,
    _phantom: PhantomData<T>,
}

impl<T> Drop for Promise<T> {
    fn drop(&mut self) {
        if !self.cancelled_or_finished.get() {
            panic!("Promise droppped without being cancelled or completed!");
        }
    }
}

#[test]
fn test_promise() {
    let mut exec = PromiseExecutor::new();
    let promise = exec.spawn(async { std::future::ready(5).await });
    assert_eq!(exec.try_get(&promise), None);
    exec.poll();
    assert_eq!(exec.try_get(&promise), Some(5));

    struct ExampleFuture {
        done: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }

    impl ExampleFuture {
        fn new() -> Self {
            let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let done_clone = done.clone();
            let _ = std::thread::spawn(move || {
                std::thread::sleep(std::time::Duration::from_millis(50));
                done_clone.store(true, std::sync::atomic::Ordering::Relaxed);
            });
            ExampleFuture { done }
        }
    }

    impl Future for ExampleFuture {
        type Output = ();

        fn poll(
            self: Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Self::Output> {
            if self.done.load(std::sync::atomic::Ordering::Relaxed) {
                std::task::Poll::Ready(())
            } else {
                std::task::Poll::Pending
            }
        }
    }

    let promise = exec.spawn(async { ExampleFuture::new().await });

    loop {
        exec.poll();

        if exec.try_get(&promise).is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
}
