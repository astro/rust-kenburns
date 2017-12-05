#[macro_use]
extern crate glium;
extern crate image;
extern crate time;
extern crate futures;
extern crate tokio_core;
extern crate hyper;
extern crate hyper_tls;
extern crate treexml;

use std::sync::mpsc::{sync_channel};
use std::thread;

mod render;
mod util;
mod http;
mod source;
mod frame_counter;

use render::*;
use source::Loader;
use frame_counter::FrameCounter;

fn main() {
    let (source_tx, source_rx) = sync_channel(0);
    let mut renderer = Renderer::new(source_rx);
    thread::spawn(move|| {
        let filenames: Vec<String> = std::env::args()
            .skip(1)
            .collect();
        Loader::new(source_tx).run_loop(filenames);
    });

    let mut counter = FrameCounter::new(1_000_000);
    while renderer.update() {
        renderer.render();
        counter.tick();
    }
}
