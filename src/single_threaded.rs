//! # Single-threaded executor
//!
//! This executor works *strictly* in a single-threaded environment. In order to spawn a task, use
//! [`spawn`]. To run the executor, use [`run`].
//!
//! There is no need to create an instance of the executor, it's automatically provisioned as a
//! thread-local instance.
//!
//! ## Example
//!
//! ```
//! use tokio::sync::*;
//! use wasm_rs_async_executor::single_threaded::{spawn, run};
//! let (sender, receiver) = oneshot::channel::<()>();
//! let _task = spawn(async move {
//!    // Complete when something is received
//!    let _ = receiver.await;
//! });
//! // Send data to be received
//! let _ = sender.send(());
//! run(None);
//! ```
use futures::channel::oneshot;
use futures::task::{waker_ref, ArcWake};
use std::cell::UnsafeCell;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

/// Task token
pub type Token = usize;

/// Task handle
#[derive(Clone)]
pub struct Task {
    token: Token,
}

impl ArcWake for Task {
    fn wake_by_ref(arc_self: &Arc<Self>) {
        EXECUTOR.with(|cell| (unsafe { &mut *cell.get() }).enqueue(arc_self.clone()))
    }
}

/// Single-threaded executor
struct Executor {
    counter: Token,
    futures: BTreeMap<Token, Pin<Box<dyn Future<Output = ()>>>>,
    #[cfg(feature = "debug")]
    types: BTreeMap<Token, String>,
    queue: Vec<Arc<Task>>,
    executing: Option<Token>,
}

impl Executor {
    fn new() -> Self {
        Self {
            counter: 0,
            futures: BTreeMap::new(),
            #[cfg(feature = "debug")]
            types: BTreeMap::new(),
            queue: vec![],
            executing: None,
        }
    }

    fn enqueue(&mut self, task: Arc<Task>) {
        if self.futures.contains_key(&task.token) {
            self.queue.insert(0, task);
        }
    }

    fn spawn<F>(&mut self, fut: F) -> Task
    where
        F: Future<Output = ()> + 'static,
    {
        self.spawn_non_static(fut)
    }

    fn spawn_non_static<F>(&mut self, fut: F) -> Task
    where
        F: Future<Output = ()>,
    {
        let token = self.counter;
        self.counter = self.counter.wrapping_add(1);
        #[cfg(feature = "debug")]
        self.types.insert(token, std::any::type_name::<F>().into());

        self.futures.insert(token, unsafe {
            Pin::new_unchecked(std::mem::transmute::<_, Box<dyn Future<Output = ()>>>(
                Box::new(fut) as Box<dyn Future<Output = ()>>,
            ))
        });
        let task = Task { token };
        self.queue.push(Arc::new(task.clone()));
        task
    }
}

thread_local! {
     static EXECUTOR: UnsafeCell<Executor> = UnsafeCell::new(Executor::new()) ;
}

thread_local! {
     static UNTIL: UnsafeCell<Option<Task>> = UnsafeCell::new(None) ;
}

thread_local! {
     static UNTIL_SATISFIED: UnsafeCell<bool> = UnsafeCell::new(false) ;
}

thread_local! {
     static YIELD: UnsafeCell<bool> = UnsafeCell::new(true) ;
}

thread_local! {
     static EXIT_LOOP: UnsafeCell<bool> = UnsafeCell::new(false) ;
}

/// Spawn a task
pub fn spawn<F>(fut: F) -> Task
where
    F: Future<Output = ()> + 'static,
{
    EXECUTOR.with(|cell| (unsafe { &mut *cell.get() }).spawn(fut))
}

/// Run tasks until completion of a future
///
/// If `cooperative` feature is enabled, given future should have `'static` lifetime.
#[cfg(not(feature = "cooperative"))]
pub fn block_on<F, R>(fut: F) -> Option<R>
where
    F: Future<Output = R>,
{
    let (sender, mut receiver) = oneshot::channel();
    let future = async move {
        let _ = sender.send(fut.await);
    };
    // We know that this task is to complete by the end of this function,
    // so let's pretend it is static
    let task = EXECUTOR.with(|cell| (unsafe { &mut *cell.get() }).spawn_non_static(future));
    run(Some(task));
    match receiver.try_recv() {
        Ok(val) => val,
        Err(_) => None,
    }
}

/// Run tasks until completion of a future
///
/// ## Important
///
/// This function WILL NOT allow yielding to the environment that `cooperative` feature allows,
/// and it will run the executor until the given future is ready. If yielding is expected,
/// this will block forever.
///
#[cfg(feature = "cooperative")]
pub fn block_on<F, R>(fut: F) -> Option<R>
where
    F: Future<Output = R>,
{
    let (sender, mut receiver) = oneshot::channel();
    let future = async move {
        let _ = sender.send(fut.await);
    };
    let task = EXECUTOR.with(|cell| (unsafe { &mut *cell.get() }).spawn_non_static(future));
    YIELD.with(|cell| unsafe {
        *cell.get() = false;
    });
    run(Some(task));
    YIELD.with(|cell| unsafe {
        *cell.get() = true;
    });
    match receiver.try_recv() {
        Ok(val) => val,
        Err(_) => None,
    }
}

/// Run the executor
///
/// If `until` is `None`, it will run until all tasks have been completed. Otherwise, it'll wait
/// until passed task is complete, or unless a `cooperative` feature has been enabled and control
/// has been yielded to the environment. In this case the function will return but the environment
/// might schedule further execution of this executor in the background after termination of the
/// function enclosing invocation of this [`run`]
pub fn run(until: Option<Task>) {
    UNTIL.with(|cell| unsafe { *cell.get() = until });
    UNTIL_SATISFIED.with(|cell| unsafe { *cell.get() = false });
    run_internal();
}

// Returns `true` if `until` task completed, or there was no `until` task and every task was
// completed.
//
// Returns `false` if loop exit was requested
fn run_internal() -> bool {
    let until = UNTIL.with(|cell| unsafe { &*cell.get() });
    let exit_condition_met = UNTIL_SATISFIED.with(|cell| unsafe { *cell.get() });
    if exit_condition_met {
        return true;
    }
    EXECUTOR.with(|cell| loop {
        let task = (unsafe { &mut *cell.get() }).queue.pop();

        if let Some(task) = task {
            let future = (unsafe { &mut *cell.get() }).futures.get_mut(&task.token);
            let ready = if let Some(future) = future {
                let waker = waker_ref(&task);
                let context = &mut Context::from_waker(&*waker);
                (unsafe { &mut *cell.get() }).executing.replace(task.token);
                let ready = matches!(future.as_mut().poll(context), Poll::Ready(_));
                (unsafe { &mut *cell.get() }).executing.take();
                ready
            } else {
                false
            };
            if ready {
                (unsafe { &mut *cell.get() }).futures.remove(&task.token);
                #[cfg(feature = "debug")]
                (unsafe { &mut *cell.get() }).types.remove(&task.token);

                if let Some(Task { ref token }) = until {
                    if *token == task.token {
                        UNTIL_SATISFIED.with(|cell| unsafe { *cell.get() = true });
                        return true;
                    }
                }
            }
        }
        if until.is_none() && (unsafe { &mut *cell.get() }).futures.is_empty() {
            UNTIL_SATISFIED.with(|cell| unsafe { *cell.get() = true });
            return true;
        }

        let exit_requested = EXIT_LOOP.with(|cell| {
            let v = cell.get();
            let result = unsafe { *v };
            // Clear the flag
            unsafe {
                *v = false;
            }
            result
        }) && YIELD.with(|cell| unsafe { *cell.get() });

        if exit_requested {
            return false;
        }

        if (unsafe { &mut *cell.get() }).queue.is_empty()
            && !(unsafe { &mut *cell.get() }).futures.is_empty()
        {
            // the executor is starving
            for token in (unsafe { &mut *cell.get() }).futures.keys() {
                (unsafe { &mut *cell.get() }).enqueue(Arc::new(Task { token: *token }));
            }
        }
    })
}

#[cfg(feature = "cooperative")]
mod cooperative {
    use super::{run_internal, EXIT_LOOP};
    use pin_project::pin_project;
    use std::cell::UnsafeCell;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::task::{Context, Poll};
    use std::time::Duration;
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_name = "setTimeout")]
        fn set_timeout(_: JsValue, delay: u32);

        #[cfg(feature = "requestIdleCallback")]
        #[wasm_bindgen(js_name = "requestIdleCallback")]
        fn request_idle_callback(_: JsValue, options: &JsValue);

        #[cfg(feature = "cooperative-browser")]
        #[wasm_bindgen(js_name = "requestAnimationFrame")]
        fn request_animation_frame(_: JsValue);

    }

    #[pin_project]
    struct TimeoutYield<F, O>
    where
        F: Future<Output = O> + 'static,
    {
        yielded: bool,
        duration: Option<Duration>,
        done: bool,
        output: Option<O>,
        #[pin]
        future: F,
        ready: Arc<UnsafeCell<bool>>,
    }

    impl<F, O> Future for TimeoutYield<F, O>
    where
        F: Future<Output = O> + 'static,
    {
        type Output = O;
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.done {
                return Poll::Pending;
            }
            if self.yielded && !unsafe { *self.ready.get() } {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
            let should_yield = !self.yielded;
            let this = self.project();
            if *this.yielded && unsafe { *this.ready.get() } && this.output.is_some() {
                // it's ok to unwrap here because we check `is_some` above
                let output = this.output.take().unwrap();
                *this.done = true;
                return Poll::Ready(output);
            }
            match (should_yield, this.future.poll(cx)) {
                (_, result @ Poll::Pending) | (true, result) => {
                    *this.yielded = true;
                    if cfg!(target_arch = "wasm32") {
                        // If this timeout is not immediate,
                        // return control to the executor at the earliest opportunity
                        if let Some(duration) = this.duration {
                            if duration.as_millis() > 0 {
                                set_timeout(
                                    Closure::once_into_js(move || {
                                        run_internal();
                                    }),
                                    0,
                                );
                            }
                        }

                        if should_yield {
                            let ready = this.ready.clone();

                            set_timeout(
                                Closure::once_into_js(move || {
                                    unsafe { *ready.get() = true };
                                    run_internal();
                                }),
                                this.duration
                                    .unwrap_or(Duration::from_millis(0))
                                    .as_millis() as u32,
                            );
                        }
                        EXIT_LOOP.with(|cell| unsafe { *cell.get() = true });
                    }
                    if let Poll::Ready(output) = result {
                        this.output.replace(output);
                    }
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
                (false, Poll::Ready(output)) => {
                    *this.done = true;
                    Poll::Ready(output)
                }
            }
        }
    }

    /// Yields the JavaScript environment using `setTimeout` function
    ///
    /// This future will be complete after `duration` has passed
    ///
    /// Only available under `cooperative` feature gate
    ///
    /// ## Caution
    ///
    /// Specifying a non-zero timeout duration will result in the executor not
    /// being called for that duration or longer.
    pub fn yield_timeout(duration: Duration) -> impl Future<Output = ()> {
        TimeoutYield {
            future: futures::future::ready(()),
            duration: Some(duration),
            output: None,
            yielded: false,
            done: false,
            ready: Arc::new(UnsafeCell::new(false)),
        }
    }

    /// Yields a future to the JavaScript environment using `setTimeout` function
    ///
    /// This future will be ready after yielding and when the enclosed future is ready.
    ///
    /// Only available under `cooperative` feature gate
    pub fn yield_async<F, O>(future: F) -> impl Future<Output = O>
    where
        F: Future<Output = O> + 'static,
    {
        TimeoutYield {
            future,
            duration: None,
            output: None,
            yielded: false,
            done: false,
            ready: Arc::new(UnsafeCell::new(false)),
        }
    }

    #[cfg(feature = "cooperative-browser")]
    #[pin_project]
    struct AnimationFrameYield {
        yielded: bool,
        done: bool,
        output: Arc<UnsafeCell<Option<f64>>>,
    }

    #[cfg(feature = "cooperative-browser")]
    impl Future for AnimationFrameYield {
        type Output = f64;
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.done {
                return Poll::Pending;
            }
            let should_yield = !self.yielded;
            let this = self.project();
            if *this.yielded && unsafe { &*this.output.get() }.is_some() {
                // it's ok to unwrap here because we check `is_some` above
                let output = unsafe { &mut *this.output.get() }.take().unwrap();
                *this.done = true;
                return Poll::Ready(output);
            }

            if should_yield {
                *this.yielded = true;
                if cfg!(target_arch = "wasm32") {
                    let output = this.output.clone();
                    request_animation_frame(Closure::once_into_js(move |timestamp| {
                        unsafe { &mut *output.get() }.replace(timestamp);
                        run_internal();
                    }));
                    EXIT_LOOP.with(|cell| unsafe { *cell.get() = true });
                }
            }

            cx.waker().wake_by_ref();

            Poll::Pending
        }
    }

    /// Yields to the browser using `requestAnimationFrame`
    ///
    /// This allows to yield to the browser until the next animation frame is requested to be
    /// rendered.
    ///
    /// It will output high resolution timer as
    /// [requestAnimationFrame](https://developer.mozilla.org/en-US/docs/Web/API/window/requestAnimationFrame)
    ///
    /// Only available under `cooperative-browser` feature gate
    ///
    #[cfg(feature = "cooperative-browser")]
    pub fn yield_animation_frame() -> impl Future<Output = f64> {
        AnimationFrameYield {
            output: Arc::new(UnsafeCell::new(None)),
            yielded: false,
            done: false,
        }
    }

    #[cfg(feature = "requestIdleCallback")]
    #[pin_project]
    struct UntilIdleYield {
        timeout: Option<Duration>,
        yielded: bool,
        done: bool,
        output: Arc<UnsafeCell<Option<web_sys::IdleDeadline>>>,
    }

    #[cfg(feature = "requestIdleCallback")]
    impl Future for UntilIdleYield {
        type Output = web_sys::IdleDeadline;
        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.done {
                return Poll::Pending;
            }
            let should_yield = !self.yielded;
            let this = self.project();
            if *this.yielded && unsafe { &*this.output.get() }.is_some() {
                // it's ok to unwrap here because we check `is_some` above
                let output = unsafe { &mut *this.output.get() }.take().unwrap();
                *this.done = true;
                return Poll::Ready(output);
            }

            if should_yield {
                *this.yielded = true;
                if cfg!(target_arch = "wasm32") {
                    let map = js_sys::Map::new();
                    if let Some(timeout) = this.timeout {
                        map.set(&"timeout".into(), &(timeout.as_millis() as u32).into());
                    }
                    let options =
                        js_sys::Object::from_entries(&map).unwrap_or(js_sys::Object::new());
                    let output = this.output.clone();
                    request_idle_callback(
                        Closure::once_into_js(move |timestamp| {
                            unsafe { &mut *output.get() }.replace(timestamp);
                            run_internal();
                        }),
                        &options.into(),
                    );
                    EXIT_LOOP.with(|cell| unsafe { *cell.get() = true });
                }
            }

            cx.waker().wake_by_ref();

            Poll::Pending
        }
    }

    /// Yields to the browser using `requestIdleCallback`
    ///
    /// This allows to yield to the browser until browser is delayed.
    ///
    /// It will output [`web_sys::IdleDeadline`] as per
    /// [requestIdleCallback](https://developer.mozilla.org/en-US/docs/Web/API/Window/requestIdleCallback)
    ///
    /// Only available under `requestIdleCallback` feature gate
    ///
    #[cfg(feature = "requestIdleCallback")]
    pub fn yield_until_idle(
        timeout: Option<Duration>,
    ) -> impl Future<Output = web_sys::IdleDeadline> {
        UntilIdleYield {
            timeout,
            output: Arc::new(UnsafeCell::new(None)),
            yielded: false,
            done: false,
        }
    }
}

#[cfg(feature = "cooperative")]
pub use cooperative::*;

/// Returns the number of tasks currently registered with the executor
pub fn tasks() -> usize {
    EXECUTOR.with(|cell| {
        let executor = unsafe { &mut *cell.get() };
        executor.futures.len()
    })
}

/// Returns the number of tasks currently in the queue to execute
pub fn queued_tasks() -> usize {
    EXECUTOR.with(|cell| (unsafe { &mut *cell.get() }).queue.len())
}

/// Returns tokens for all tasks that haven't completed yet
///
/// ## Note
///
/// Enabled only when `debug` feature is turned on
#[cfg(feature = "debug")]
pub fn tokens() -> Vec<Token> {
    EXECUTOR.with(|cell| (unsafe { &*cell.get() }).types.keys().map(|k| *k).collect())
}

/// Returns tokens for queued tasks
///
/// ## Note
///
/// Enabled only when `debug` feature is turned on
#[cfg(feature = "debug")]
pub fn queued_tokens() -> Vec<Token> {
    EXECUTOR.with(|cell| {
        (unsafe { &*cell.get() })
            .queue
            .iter()
            .map(|t| t.token)
            .collect()
    })
}

/// Returns task's future type for a given token
///
/// Useful for introspection into current task list.
///
/// ## Note
///
/// Enabled only when `debug` feature is turned on
#[cfg(feature = "debug")]
pub fn task_type(token: Token) -> Option<&'static String> {
    EXECUTOR.with(|cell| (unsafe { &*cell.get() }).types.get(&token))
}

/// Removes all tasks from the executor
///
/// ## Caution
///
/// Evicted tasks won't be able to get re-scheduled when they will be woken up.
pub fn evict_all() {
    EXECUTOR.with(|cell| unsafe { *cell.get() = Executor::new() });
}

#[cfg(test)]
fn set_counter(counter: usize) {
    EXECUTOR.with(|cell| (unsafe { &mut *cell.get() }).counter = counter);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::*;

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test() {
        use tokio::sync::*;
        let (sender, receiver) = oneshot::channel::<()>();
        let _task = spawn(async move {
            let _ = receiver.await;
        });
        let _ = sender.send(());
        run(None);
        evict_all();
    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_until() {
        use tokio::sync::*;
        let (_sender1, receiver1) = oneshot::channel::<()>();
        let _task1 = spawn(async move {
            let _ = receiver1.await;
        });
        let (sender2, receiver2) = oneshot::channel::<()>();
        let task2 = spawn(async move {
            let _ = receiver2.await;
        });
        let _ = sender2.send(());
        run(Some(task2));
        evict_all();
    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn test_counts() {
        use tokio::sync::*;
        let (sender, mut receiver) = oneshot::channel();
        let (sender2, receiver2) = oneshot::channel::<()>();
        let task1 = spawn(async move {
            let _ = receiver2.await;
            let _ = sender.send((tasks(), queued_tasks()));
        });
        let _task2 = spawn(async move {
            let _ = sender2.send(());
            futures::future::pending().await // this will never end
        });
        run(Some(task1));
        let (tasks_, queued_tasks_) = receiver.try_recv().unwrap();
        // task1 + task2
        assert_eq!(tasks_, 2);
        // task1 is being executed, task2 has nothing new
        assert_eq!(queued_tasks_, 0);
        // task1 is gone
        assert_eq!(tasks(), 1);
        // task2 still has nothing new
        assert_eq!(queued_tasks(), 0);
        evict_all();
    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn evicted_tasks_dont_requeue() {
        use tokio::sync::*;
        let (_sender, receiver) = oneshot::channel::<()>();
        let task = spawn(async move {
            let _ = receiver.await;
        });
        assert_eq!(tasks(), 1);
        evict_all();
        assert_eq!(tasks(), 0);
        ArcWake::wake_by_ref(&Arc::new(task));
        assert_eq!(tasks(), 0);
        assert_eq!(queued_tasks(), 0);
        evict_all();
    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn token_exhaustion() {
        set_counter(usize::MAX);
        // this should be fine anyway
        let task_0 = spawn(async move {});
        // this should NOT crash
        let task = spawn(async move {});
        // new token should be different and wrap back to the beginning
        assert!(task.token != task_0.token);
        assert_eq!(task.token, 0);
        evict_all();
    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn blocking_on() {
        use tokio::sync::*;
        let (sender, receiver) = oneshot::channel::<u8>();
        let _task = spawn(async move {
            let _ = sender.send(1);
        });
        let result = block_on(async move { receiver.await.unwrap() });
        assert_eq!(result.unwrap(), 1);
        evict_all();
    }

    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn starvation() {
        use tokio::sync::*;
        let (sender, receiver) = oneshot::channel();
        let _task = spawn(async move {
            tokio::task::yield_now().await;
            tokio::task::yield_now().await;
            let _ = sender.send(());
        });
        let result = block_on(async move { receiver.await.unwrap() });
        assert_eq!(result.unwrap(), ());
        evict_all();
    }

    #[cfg(feature = "debug")]
    #[cfg_attr(not(target_arch = "wasm32"), test)]
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    fn task_type_info() {
        spawn(futures::future::pending());
        assert!(task_type(tokens()[0])
            .unwrap()
            .contains("future::pending::Pending"));
        evict_all();
        assert_eq!(tokens().len(), 0);
    }
}
