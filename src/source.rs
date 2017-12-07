use std::fs::{metadata, File, read_dir, DirEntry};
use std::io::{BufReader, Read};
use std::sync::mpsc::SyncSender;
use glium::texture::RawImage2d;
use image::{jpeg, ImageDecoder, DynamicImage, ImageResult};
use hyper::Uri;
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
                let uri = match filename.parse() {
                    Ok(uri) => uri,
                    Err(_) => return,
                };
                let res = match get(&uri) {
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
                    let body = res.body();
                    self.load_jpeg(BufReader::new(body))
                } else if is_feed {
                    println!("Reading feed til end...");
                    let body = res.body();
                    self.run_feed(&uri, body)
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

    fn run_feed<R: Read>(&self, base: &Uri, res: R) {
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
                        self.load_feed_item(base, item);
                    }
                }
                /* ATOM */
                for entry in root.filter_children(|el| el.name == "entry") {
                    self.load_feed_item(base, entry);
                }
            },
            Ok(_) => println!("Error parsing XML: no root element!"),
            Err(e) => println!("Error parsing XML: {}", e)
        }
    }

    fn load_feed_item(&self, base: &Uri, item: &treexml::Element) {
        let run_link = |href| {
            uri_join(base, href)
                .map(|url| self.run_filename(&url));
        };
        /* <atom:link rel="enclosure" href="http://..."/> */
        for content in item.filter_children(|el| el.name == "link") {
            match (content.attributes.get("rel"), content.attributes.get("href")) {
                (Some(rel), Some(url)) if rel == "enclosure" => {
                    run_link(url);
                    return
                },
                _ => ()
            }
        }
        /* <media:content url="http://..."/> */
        for content in item.filter_children(|el| el.name == "content") {
            match content.attributes.get("url") {
                Some(url) => {
                    run_link(url);
                    return
                },
                None => ()
            }
        }
    }

    pub fn load_jpeg<R: Read>(&self, file: R) -> () {
        let t1 = get_us();
        println!("Load JPEG...");
        let image = match decoder_to_image(jpeg::JPEGDecoder::new(file)) {
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

/// From image::dynimage (private)
/// 
/// Decodes an image and stores it into a dynamic image
pub fn decoder_to_image<I: ImageDecoder>(codec: I) -> ImageResult<DynamicImage> {
    use std::iter;
    use image;
    use image::{DynamicImage, ImageBuffer, ColorType};
    use image::DecodingResult::U8;
    use num_iter;

    let mut codec = codec;

    let color  = try!(codec.colortype());
    let buf    = try!(codec.read_image());
    let (w, h) = try!(codec.dimensions());

    let image = match (color, buf) {
        (ColorType::RGB(8), U8(buf)) => {
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageRgb8)
        }

        (ColorType::RGBA(8), U8(buf)) => {
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageRgba8)
        }

        (ColorType::Gray(8), U8(buf)) => {
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageLuma8)
        }

        (ColorType::GrayA(8), U8(buf)) => {
            ImageBuffer::from_raw(w, h, buf).map(DynamicImage::ImageLumaA8)
        }
        (ColorType::Gray(bit_depth), U8(ref buf)) if bit_depth == 1 || bit_depth == 2 || bit_depth == 4 => {
            // Note: this conversion assumes that the scanlines begin on byte boundaries
            let mask = (1u8 << bit_depth as usize) - 1;
            let scaling_factor = 255/((1 << bit_depth as usize) - 1);
            let skip = (w % 8)/u32::from(bit_depth);
            let row_len = w + skip;
            let p = buf
                       .iter()
                       .flat_map(|&v|
                           num_iter::range_step_inclusive(8i8-(bit_depth as i8), 0, -(bit_depth as i8))
                           .zip(iter::repeat(v))
                       )
                       // skip the pixels that can be neglected because scanlines should
                       // start at byte boundaries
                       .enumerate().filter(|&(i, _)| i % (row_len as usize) < (w as usize) ).map(|(_, p)| p)
                       .map(|(shift, pixel)|
                           (pixel & mask << shift as usize) >> shift as usize
                       )
                       .map(|pixel| pixel * scaling_factor)
                       .collect();
            ImageBuffer::from_raw(w, h, p).map(DynamicImage::ImageLuma8)
        },
        _ => return Err(image::ImageError::UnsupportedColor(color))
    };
    match image {
        Some(image) => Ok(image),
        None => Err(image::ImageError::DimensionError)
    }
}

/// Incomplete
fn uri_join(base: &Uri, href: &str) -> Option<String> {
    if href.starts_with("http://") || href.starts_with("https://")  {
        Some(href.to_owned())
    } else if href.starts_with("/") {
        base.scheme()
            .and_then(
                |scheme| base.authority()
                    .map(
                        |authority|
                        format!("{}://{}{}", scheme, authority, href)
                    )
            )
    } else {
        // Incomplete!
        base.scheme()
            .and_then(
                |scheme| base.authority()
                    .map(
                        |authority|
                        format!("{}://{}/{}", scheme, authority, href)
                    )
            )
    }
}
