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
use futures::task::{waker_ref, ArcWake};
use std::cell::UnsafeCell;
use std::collections::BTreeMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

/// Task token
type Token = usize;

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
    futures: BTreeMap<Token, Box<dyn Future<Output = ()>>>,
    queue: Vec<Arc<Task>>,
}

impl Executor {
    fn new() -> Self {
        Self {
            counter: 0,
            futures: BTreeMap::new(),
            queue: vec![],
        }
    }

    fn enqueue(&mut self, task: Arc<Task>) {
        self.queue.insert(0, task);
    }

    fn spawn<F>(&mut self, fut: F) -> Task
    where
        F: Future<Output = ()> + 'static,
    {
        let token = self.counter;
        self.counter += 1;
        self.futures.insert(token, Box::new(fut));
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

/// Run the executor
///
/// If `until` is `None`, it will run until all tasks have been completed. Otherwise, it'll wait
/// until passed task is complete.
pub fn run(until: Option<Task>) {
    EXECUTOR.with(|cell| loop {
        let task = (unsafe { &mut *cell.get() }).queue.pop();

        if let Some(task) = task {
            let future = (unsafe { &mut *cell.get() }).futures.remove(&task.token);
            if let Some(future) = future {
                let waker = waker_ref(&task);
                let context = &mut Context::from_waker(&*waker);
                let mut future = unsafe { Pin::new_unchecked(future) };
                if let Poll::Pending = future.as_mut().poll(context) {
                    (unsafe { &mut *cell.get() })
                        .futures
                        .insert(task.token, unsafe { Pin::into_inner_unchecked(future) });
                } else if let Some(Task { ref token }) = until {
                    if *token == task.token {
                        return;
                    }
                }
            }
        }
        if until.is_none() && (unsafe { &mut *cell.get() }).futures.is_empty() {
            return;
        }
    })
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
    }
}
