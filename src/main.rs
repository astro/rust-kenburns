#[macro_use]
extern crate glium;
extern crate image;
extern crate time;

use std::thread;
use std::sync::mpsc::{sync_channel};

mod render;
mod util;
mod source;

use util::*;
use render::*;

fn main() {
    let (source_tx, source_rx) = sync_channel(0);
    let mut renderer = Renderer::new(source_rx);
    thread::spawn(move|| {
        let filenames: Vec<String> = std::env::args()
            .skip(1)
            .collect();
        source::run(filenames, source_tx);
    });
    
    while renderer.update() {
        renderer.render();
    }
}
