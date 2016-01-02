use std::fs::File;
use std::sync::mpsc::SyncSender;
use glium::texture::RawImage2d;
use image;

use util::*;

pub fn run<'a>(filenames: Vec<String>, tx: SyncSender<RawImage2d<'a, u8>>) {
    loop {
        for filename in &filenames {
            println!("Loading file {}...", filename);
            let t1 = get_us();
            let file = File::open(filename).unwrap();
            let t2 = get_us();
            let image = image::load(file, image::JPEG).unwrap().to_rgba();
            let t3 = get_us();
            let image_dimensions = image.dimensions();
            let t4 = get_us();
            let image = RawImage2d::from_raw_rgba_reversed(image.into_raw(), image_dimensions);
            let t5 = get_us();
            println!("Loaded {} in {} + {} + {} us", filename, t2 - t1, t3 - t2, t5 - t4);
            tx.send(image);
        }
    }
}
