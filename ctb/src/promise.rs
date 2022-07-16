use slotmap::SlotMap;
use std::{any::Any, cell::Cell, future::Future, marker::PhantomData, pin::Pin};

fn null_waker() -> std::task::Waker {
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
            &() as *const (),
            &std::task::RawWakerVTable::new(_nothing1, _nothing2, _nothing3, _nothing4),
        ))
    }
}

type Fut = dyn Future<Output = Box<dyn Any>>;
enum FutureOrValue {
    Future(Pin<Box<Fut>>),
    Value(Box<dyn Any>),
}

pub struct PromiseFuture {
    futval: FutureOrValue,
    detached: bool,
}

pub struct PromiseExecutor {
    promises: SlotMap<slotmap::DefaultKey, PromiseFuture>,
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
        Promise {
            cancelled_or_finished: Cell::new(false),
            id: self.insert_promise(fut, false),
            _phantom: PhantomData,
        }
    }

    pub fn spawn_detached<T: Send + 'static, F>(&mut self, fut: F)
    where
        F: Future<Output = T> + 'static,
    {
        self.insert_promise(fut, true);
    }

    pub fn insert_promise<T: Send + 'static, F>(
        &mut self,
        fut: F,
        detached: bool,
    ) -> slotmap::DefaultKey
    where
        F: Future<Output = T> + 'static,
    {
        self.promises.insert(PromiseFuture {
            futval: FutureOrValue::Future(Box::pin(async { Box::new(fut.await) as Box<dyn Any> })),
            detached,
        })
    }

    pub fn try_get<T: 'static>(&mut self, promise: &Promise<T>) -> Option<T> {
        if promise.cancelled_or_finished.get() {
            panic!("Can't poll an already finished Promise.");
        }

        match self.promises[promise.id].futval {
            FutureOrValue::Future(_) => None,
            FutureOrValue::Value(_) => {
                promise.cancelled_or_finished.set(true);
                match self.promises.remove(promise.id).unwrap().futval {
                    FutureOrValue::Future(_) => unreachable!(),
                    FutureOrValue::Value(v) => Some(*v.downcast().unwrap()),
                }
            }
        }
    }

    pub fn poll(&mut self) {
        let mut unretrieved_values = 0;
        let keys = self.promises.keys().collect::<Vec<_>>();
        for key in keys {
            match &mut self.promises[key].futval {
                FutureOrValue::Future(f) => {
                    let waker = null_waker();
                    let mut cx = std::task::Context::from_waker(&waker);
                    match std::future::Future::poll(f.as_mut(), &mut cx) {
                        std::task::Poll::Ready(v) => {
                            if self.promises[key].detached {
                                self.promises.remove(key).unwrap();
                            } else {
                                self.promises[key].futval = FutureOrValue::Value(v);
                            }
                        }
                        std::task::Poll::Pending => {}
                    }
                }
                FutureOrValue::Value(_) => {
                    unretrieved_values += 1;
                }
            }
        }

        if unretrieved_values >= 10 {
            println!(
                "Too many unretrieved values ({}). Something must've gone wrong somewhere..",
                unretrieved_values
            );
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
