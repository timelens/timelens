extern crate image;

use std::path::Path;
use std::fs::File;

fn main() {
    let num = 50;
    let mut bg = image::ImageBuffer::new(num, 100);
    //let mut bg = image::DynamicImage::new_rgb8(num, 100);
    for i in 0..num {
        let img = image::open(&Path::new(format!("frame{:04}.png", i).as_str())).unwrap();
        let img2 = image::imageops::resize(&img, 1, 100, image::FilterType::Triangle);
        image::imageops::overlay(&mut bg, &img2, i, 0);
    }
    let ref mut fout = File::create("result.png").unwrap();
    bg.save("result.png").unwrap();
}
