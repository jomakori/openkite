//! The browser half: hydrate the SSR markup from wasm and refresh through
//! `POST /api/gateway`.
//!
//! The initial snapshot is embedded in the page by the host. Re-parsing it
//! on boot means the same VirtualDom walks the same tree the server rendered,
//! so hydration matches by construction; every later refresh flows through
//! the server-side gateway.

use dioxus::prelude::VirtualDom;
use wasm_bindgen::prelude::*;

use openkite_web::app::{App, AppProps};
use openkite_web::ssr::Snapshot;

#[wasm_bindgen(start)]
pub fn start() {
    let initial = parse_snapshot().unwrap_or_default();
    let vdom = VirtualDom::new_with_props(App, AppProps { snapshot: initial });
    dioxus_web::launch::launch_virtual_dom(vdom, dioxus_web::Config::new().hydrate(true));
}

fn parse_snapshot() -> Option<Snapshot> {
    let window = web_sys::window()?;
    let document = window.document()?;
    let element = document
        .query_selector("#openkite-snapshot")
        .ok()
        .flatten()?;
    let text = element.text_content()?;
    serde_json::from_str(&text).ok()
}
