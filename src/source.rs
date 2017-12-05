use std::panic::set_hook;
use std::fs::{metadata, File, read_dir, DirEntry};
use std::io::{BufRead, BufReader, Read, Seek, Cursor};
use std::sync::mpsc::SyncSender;
use glium::texture::RawImage2d;
use image;
use hyper::header::ContentType;
use hyper::mime::{IMAGE_JPEG, TEXT_XML, APPLICATION};
use treexml;

use util::*;
use http::get;

pub struct Loader<'a> {
    tx: SyncSender<RawImage2d<'a, u8>>,
}

/**
 * * run*() methods: iterate over files
 * * load*() methods: load one file
 **/
impl<'a> Loader<'a> {
    pub fn new(tx: SyncSender<RawImage2d<'a, u8>>) -> Loader<'a> {
        Loader {
            tx: tx,
        }
    }

    pub fn run_loop(&self, filenames: Vec<String>) {
        set_hook(Box::new(|info| println!("Loader panic: {:?}", info)));
        
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
                let res = match get(&filename) {
                    Err(e) => {
                        println!("{}", e);
                        return
                    },
                    Ok(res) => res
                };
                println!("HTTP {}: {:?}", res.status(), res.headers().get::<ContentType>());
                let (is_jpeg, is_feed) = match res.headers().get::<ContentType>() {
                    Some(&ContentType(ref mime)) if mime == &IMAGE_JPEG =>
                        (true, false),
                    Some(&ContentType(ref mime)) if mime == &TEXT_XML =>
                        (false, true),
                    Some(&ContentType(ref mime))
                        if mime.type_() == APPLICATION &&
                        mime.suffix()
                        .map(|name| name.as_str()) == Some("xml") =>
                        (false, true),
                    Some(&ContentType(ref mime)) => {
                        println!("Cannot handle content-type {}", mime);
                        (false, false)
                    },
                    None =>
                        (false, false),
                };
                if is_jpeg {
                    println!("Reading JPEG til end...");
                    match res.body() {
                            Ok(body) =>
                                self.load_jpeg(Cursor::new(body)),
                            Err(e) =>
                                println!("Error loading: {}", e)
                    };
                } else if is_feed {
                    println!("Reading feed til end...");
                    match res.body() {
                            Ok(body) => {
                                println!("body: {}", body.len());
                                self.run_feed(Cursor::new(body))
                            },
                            Err(e) =>
                                println!("Error loading: {}", e)
                    };
                }
            } else {
                let attr = metadata(filename).unwrap();
                if attr.is_file() || attr.file_type().is_symlink() {
                    let lower_filename = filename.to_lowercase();
                    if lower_filename.ends_with(".jpg") ||
                        lower_filename.ends_with(".jpeg") {

                            let file = File::open(filename).unwrap();
                            self.load_jpeg(BufReader::new(file));
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
        match treexml::Document::parse(res) {
            Ok(treexml::Document {
                root: Some(root),
                version: _,
                encoding: _
            }) => {
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
            },
            Ok(_) => println!("Error parsing XML: no root element!"),
            Err(e) => println!("Error parsing XML: {}", e)
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

    pub fn load_jpeg<R: BufRead + Read + Seek>(&self, file: R) -> () {
        let t1 = get_us();
        println!("Load JPEG...");
        let image = match image::load(file, image::JPEG) {
            Ok(image) => { println!("Loaded image!"); image.to_rgba() },
            Err(e) => {
                println!("Error loading JPEG: {}", e);
                return
            }
        };
        let t2 = get_us();
        let image_dimensions = image.dimensions();
        let t3 = get_us();
        let image = RawImage2d::from_raw_rgba_reversed(&image.into_raw(), image_dimensions);
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
