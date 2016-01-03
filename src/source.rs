use std::fs::{metadata, File, read_dir, DirEntry};
use std::io::{Read, Seek, Cursor};
use std::sync::mpsc::SyncSender;
use glium::texture::RawImage2d;
use image;
use hyper;
use hyper::mime::{Mime, TopLevel, SubLevel};
use treexml;

use util::*;

pub struct Loader<'a> {
    tx: SyncSender<RawImage2d<'a, u8>>,
    http: hyper::client::Client
}

impl<'a> Loader<'a> {
    pub fn new(tx: SyncSender<RawImage2d<'a, u8>>) -> Loader<'a> {
        Loader {
            tx: tx,
            http: hyper::client::Client::new()
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

            println!("GET {}", filename);
            let res = self.http.get(filename).send().unwrap();
            println!("HTTP {}: {:?}", res.status, res.headers.get::<hyper::header::ContentType>());
            let (is_jpeg, is_feed) = match res.headers.get::<hyper::header::ContentType>() {
                Some(&hyper::header::ContentType(Mime(TopLevel::Image, SubLevel::Jpeg, _))) =>
                    (true, false),
                Some(&hyper::header::ContentType(Mime(TopLevel::Text, SubLevel::Xml, _))) =>
                    (false, true),
                _ =>
                    (false, false)
            };
            if is_jpeg {
                self.load_jpeg(Cursor::<Vec<u8>>::new(res.bytes().map(|b| b.unwrap()).collect()));
            } else if is_feed {
                let doc = treexml::Document::parse(res).unwrap();
                let root = doc.root.unwrap();
                for channel in root.filter_children(|el| el.name == "channel") {
                    for item in channel.filter_children(|el| el.name == "item") {
                        for content in item.filter_children(|el| el.name == "content") {
                            match content.attributes.get("url") {
                                Some(url) => self.run_filename(url),
                                None => ()
                            }
                        }
                    }
                }
            }
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
