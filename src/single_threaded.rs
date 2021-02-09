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
#[cfg(feature = "cooperative")]
use wasm_bindgen::prelude::*;

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

/// Spawn a task
pub fn spawn<F>(fut: F) -> Task
where
    F: Future<Output = ()> + 'static,
{
    EXECUTOR.with(|cell| (unsafe { &mut *cell.get() }).spawn(fut))
}

/// Run tasks until completion of a future
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

/// Run the executor
///
/// If `until` is `None`, it will run until all tasks have been completed. Otherwise, it'll wait
/// until passed task is complete.
pub fn run(until: Option<Task>) {
    run_max(&until, None);
}

// Returns `true` if `until` task completed, or there was no `until` task and every task was
// completed.
//
// It returns `false` if `max` was reached
fn run_max(until: &Option<Task>, max: Option<usize>) -> bool {
    let mut counter = 0;
    EXECUTOR.with(|cell| loop {
        if let Some(c) = max {
            if c == counter {
                return false;
            }
        }
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
                        return true;
                    }
                }
            }
        }
        if until.is_none() && (unsafe { &mut *cell.get() }).futures.is_empty() {
            return true;
        }
        if (unsafe { &mut *cell.get() }).queue.is_empty()
            && !(unsafe { &mut *cell.get() }).futures.is_empty()
        {
            // the executor is starving
            for token in (unsafe { &mut *cell.get() }).futures.keys() {
                (unsafe { &mut *cell.get() }).enqueue(Arc::new(Task { token: *token }));
            }
        }
        counter += 1
    })
}

#[cfg(feature = "cooperative")]
#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = "setTimeout")]
    fn set_timeout(_: JsValue);
}

/// Runs the scheduled cooperatively with JavaScript environment
///
/// This function will return immediately after one iterator of task queue processing
/// but it will schedule its own execution if the desired outcome wasn't reached, allowing
/// JavaScript event loop to proceed.
///
/// This function is available under `cooperative` feature gate.
#[cfg(feature = "cooperative")]
pub fn run_cooperatively(until: Option<Task>) {
    if !run_max(&until, Some(1)) {
        set_timeout(Closure::once_into_js(|| run_cooperatively(until)));
    }
}

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
