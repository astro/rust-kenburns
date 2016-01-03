use std::fs::{metadata, File, read_dir, DirEntry};
use std::io::{Read, Seek};
use std::sync::mpsc::SyncSender;
use glium::texture::RawImage2d;
use image;

use util::*;

pub struct Loader<'a> {
    tx: SyncSender<RawImage2d<'a, u8>>
}

impl<'a> Loader<'a> {
    pub fn new(tx: SyncSender<RawImage2d<'a, u8>>) -> Loader<'a> {
        Loader {
            tx: tx
        }
    }

    pub fn run_loop(&self, filenames: Vec<String>) {
        loop {
            for filename in &filenames {
                self.run_filename(filename);
            }
        }
    }


    pub fn run_filename(&self, filename: &str) {
        if filename.starts_with("http://") ||
            filename.starts_with("https://") {
        } else {
            let attr = metadata(filename).unwrap();
            if attr.is_file() || attr.file_type().is_symlink() {
                let lower_filename = filename.to_lowercase();
                if lower_filename.ends_with(".jpg") ||
                    lower_filename.ends_with(".jpeg") {

                    let file = File::open(filename).unwrap();
                    self.load_jpeg(file);
                }
            } else if attr.is_dir() {
                let mut entries: Vec<(String, DirEntry)> = read_dir(filename)
                    .unwrap()
                    .map(|entry| {
                        let entry = entry.unwrap();
                        /* Create case-insensitive sort key */
                        (entry.path().to_str().unwrap().to_lowercase(), entry)
                    })
                    .collect();
                entries.sort_by(|&(ref a, _), &(ref b, _)| a.cmp(b));
                for (_, entry) in entries {
                    self.run_filename(entry.path().to_str().unwrap());
                }
            }
        }
    }

    pub fn load_jpeg<R: Read + Seek>(&self, file: R) {
        let t1 = get_us();
        let image = image::load(file, image::JPEG).unwrap().to_rgba();
        let t2 = get_us();
        let image_dimensions = image.dimensions();
        let t3 = get_us();
        let image = RawImage2d::from_raw_rgba_reversed(image.into_raw(), image_dimensions);
        let t4 = get_us();
        println!("Loaded image in {} + {} + {} us", t2 - t1, t3 - t2, t4 - t3);
        self.tx.send(image).unwrap();
    }
}
