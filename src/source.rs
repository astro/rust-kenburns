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

/**
 * * run*() methods: iterate over files
 * * load*() methods: load one file
 **/
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
            let mut res = match self.http.get(filename).send() {
                Err(e) => {
                    println!("{}", e);
                    return
                },
                Ok(res) => res
            };
            println!("HTTP {}: {:?}", res.status, res.headers.get::<hyper::header::ContentType>());
            let (is_jpeg, is_feed) = match res.headers.get::<hyper::header::ContentType>() {
                Some(&hyper::header::ContentType(Mime(TopLevel::Image, SubLevel::Jpeg, _))) =>
                    (true, false),
                Some(&hyper::header::ContentType(Mime(TopLevel::Text, SubLevel::Xml, _))) =>
                    (false, true),
                Some(&hyper::header::ContentType(Mime(TopLevel::Application, SubLevel::Ext(ref ext), _)))
                    if ext.ends_with("+xml") =>
                    (false, true),
                _ =>
                    (false, false)
            };
            if is_jpeg {
                let mut buf = Vec::new();
                println!("Reading JPEG til end...");
                match res.read_to_end(&mut buf) {
                    Ok(_) =>
                        self.load_jpeg(Cursor::new(buf)),
                    Err(e) =>
                        println!("Error loading: {}", e)
                };
            } else if is_feed {
                self.run_feed(res);
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

    fn run_feed<R: Read>(&self, res: R) {
        println!("Reading feed and parsing...");
        let doc = treexml::Document::parse(res).unwrap();
        let root = doc.root.unwrap();
        /* RSS */
        for channel in root.filter_children(|el| el.name == "channel") {
            for item in channel.filter_children(|el| el.name == "item") {
                self.load_feed_item(item);
            }
        }
        /* ATOM */
        for entry in root.filter_children(|el| el.name == "entry") {
            self.load_feed_item(entry);
        }
    }

    fn load_feed_item(&self, item: &treexml::Element) {
        /* <atom:link rel="enclosure" href="http://..."/> */
        for content in item.filter_children(|el| el.name == "link") {
            match (content.attributes.get("rel"), content.attributes.get("href")) {
                (Some(rel), Some(url)) if rel == "enclosure" => {
                    self.run_filename(url);
                    return
                },
                _ => ()
            }
        }
        /* <media:content url="http://..."/> */
        for content in item.filter_children(|el| el.name == "content") {
            match content.attributes.get("url") {
                Some(url) => {
                    self.run_filename(url);
                    return
                },
                None => ()
            }
        }
    }

    pub fn load_jpeg<R: Read + Seek>(&self, file: R) -> () {
        let t1 = get_us();
        let image = match image::load(file, image::JPEG) {
            Ok(image) => image.to_rgba(),
            Err(e) => {
                println!("Error loading JPEG: {}", e);
                return
            }
        };
        let t2 = get_us();
        let image_dimensions = image.dimensions();
        let t3 = get_us();
        let image = RawImage2d::from_raw_rgba_reversed(image.into_raw(), image_dimensions);
        let t4 = get_us();
        println!("Loaded image in {} + {} + {} us", t2 - t1, t3 - t2, t4 - t3);
        match self.tx.send(image) {
            Ok(()) => (),
            Err(e) => {
                println!("Error transferring JPEG to main thread: {}", e);
                return
            }
        }
    }
}
