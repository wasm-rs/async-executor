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
        let element = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id("display")
            .unwrap();

        let mut ctr = 0u8;
        while ctr < 255 {
            element.set_inner_html(&format!("{}", ctr));
            ctr = ctr.wrapping_add(1);
            executor::yield_animation_frame().await;
        }
    });
    let task1x = executor::spawn(async move {
        let element = web_sys::window()
            .unwrap()
            .document()
            .unwrap()
            .get_element_by_id("display1")
            .unwrap();

        loop {
            let ts = executor::yield_animation_frame().await;
            element.set_inner_html(&format!("{}", ts));
        }
    });

    let task2 = executor::spawn(async move {
        dbg!("task 2 awaiting");
        let _ = receiver2.await;
        dbg!("task 2 fetching /");
        let fut: JsFuture = web_sys::window().unwrap().fetch_with_str("/").into();
        let response: web_sys::Response = executor::yield_timeout(None, fut).await.unwrap().into();
        let text_fut: JsFuture = response.text().unwrap().into();
        dbg!("task 2 will intentionally delay executor by 1 second");
        let text: String =
            executor::yield_timeout(Some(std::time::Duration::from_secs(1)), text_fut)
                .await
                .unwrap()
                .as_string()
                .unwrap();
        dbg!(text);
        dbg!("task 2 done");
    });
    dbg!("starting executor, sending to task1");
    let _ = sender1.send(());
    executor::run(Some(task1));
}
