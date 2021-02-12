use tokio::sync::oneshot;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use wasm_rs_async_executor::single_threaded as executor;
use wasm_rs_dbg::dbg;

#[wasm_bindgen(start)]
#[allow(unused_variables)]
pub fn start() {
    let (sender1, receiver1) = oneshot::channel();
    let (sender2, receiver2) = oneshot::channel();
    let task1 = executor::spawn(async move {
        dbg!("task 1 awaiting");
        let _ = receiver1.await;
        dbg!("task 1 -> task 2");
        let _ = sender2.send(());
        dbg!("task 1 done");
    });
    let task2 = executor::spawn(async move {
        dbg!("task 2 awaiting");
        let _ = receiver2.await;
        dbg!("task 2 fetching /");
        let fut: JsFuture = web_sys::window().unwrap().fetch_with_str("/").into();
        let response: web_sys::Response = fut.await.unwrap().into();
        let text_fut: JsFuture = response.text().unwrap().into();
        let text: String = text_fut.await.unwrap().as_string().unwrap();
        dbg!(text);
        dbg!("task 2 done");
    });
    dbg!("starting executor, sending to task1");
    let _ = sender1.send(());
    executor::run_cooperatively(
        Some(task2),
        Some(|| {
            dbg!("Done");
        }),
    );
}
