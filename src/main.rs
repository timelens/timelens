#[macro_use]
extern crate clap;
use clap::Arg;

use std::cmp;

use std::io::stdout;
use std::io::Write;

use std::fs::File;
use std::path::Path;

mod frame;
mod source;

fn main() {
    // parse the command line arguments
    let mut config = parse_config();

    let source =
        source::VideoSource::new(&config.input_filename, config.thumb_height, config.width);
    config.thumb_width = source.width;
    config.tmp_width = source.width;

    let duration = source.duration;
    let (timeline, thumbnails) = generate_timeline_and_thumbnails(&config, source);

    println!("");
    timeline.write_to(&config.timeline_filename);
    println!("-> '{}'", config.timeline_filename);
    thumbnails.write_to(&config.thumbnails_filename);
    println!("-> '{}'", config.thumbnails_filename);
    write_vtt(&config, duration);
    println!("-> '{}'", config.vtt_filename);
}

// Config objects are used to describe a single Timeline run
pub struct Config {
    width: usize,
    height: usize,
    thumb_width: usize,
    thumb_height: usize,
    thumb_columns: usize,
    input_filename: String,
    timeline_filename: String,
    thumbnails_filename: String,
    vtt_filename: String,
    tmp_width: usize,
    seek_mode: bool,
}

// generate a Config from the command line arguments
fn parse_config() -> Config {
    let matches = app_from_crate!()
        .arg(Arg::with_name("input").help("Input file").index(1))
        .arg(
            Arg::with_name("width")
                .help("Width of output")
                .short("w")
                .long("width")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("height")
                .help("Height of output")
                .short("h")
                .long("height")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("timeline")
                .help("Name of timeline output file")
                .long("timeline")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("thumbnails")
                .help("Name of thumbnails output file")
                .long("thumbnails")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("vtt")
                .help("Name of VTT output file")
                .long("vtt")
                .takes_value(true),
        )
        .arg(Arg::with_name("seek").help("Allow seeking").long("seek"))
        .get_matches();

    // default width is 1000
    let width_string = matches.value_of("width").unwrap_or("1000");
    let width: usize = width_string.parse().expect("Invalid width");

    // default height is 100
    let height_string = matches.value_of("height").unwrap_or("100");
    let height: usize = height_string.parse().expect("Invalid height");

    let input_filename = matches.value_of("input").expect("No input file specified");

    // default output filenames are extensions of the input filename
    let fallback_output = format!("{}.timeline.jpg", &input_filename);
    let timeline_filename = matches.value_of("timeline").unwrap_or(&fallback_output);
    let fallback_output2 = format!("{}.thumbnails.jpg", &input_filename);
    let thumbnails_filename = matches.value_of("thumbnails").unwrap_or(&fallback_output2);
    let fallback_output3 = format!("{}.thumbnails.vtt", &input_filename);
    let vtt_filename = matches.value_of("vtt").unwrap_or(&fallback_output3);

    Config {
        width,
        height,
        thumb_width: 0,
        thumb_height: height,
        thumb_columns: 20,
        input_filename: String::from(input_filename),
        timeline_filename: String::from(timeline_filename),
        thumbnails_filename: String::from(thumbnails_filename),
        vtt_filename: String::from(vtt_filename),
        tmp_width: 0,
        seek_mode: matches.is_present("seek"),
    }
}

// convert milliseconds to a WebVTT timestamp (which has the format "(HH:)MM:SS.mmmm")
fn timestamp(mseconds_total: i32) -> String {
    let minutes = mseconds_total / (1000 * 60);
    let seconds = (mseconds_total - 1000 * 60 * minutes) / 1000;
    let mseconds = mseconds_total - 1000 * (seconds + 60 * minutes);
    format!("{:02}:{:02}.{:03}", minutes, seconds, mseconds)
}

// the hard part: actually create timeline and thumbnails file
fn generate_timeline_and_thumbnails(
    config: &Config,
    source: source::VideoSource,
) -> (frame::Frame, frame::Frame) {
    let mut timeline = frame::Frame::new(config.width, config.height);

    let thumb_rows = config.width / config.thumb_columns + 1;
    let mut thumbnails = frame::Frame::new(
        config.thumb_width * config.thumb_columns,
        config.thumb_height * thumb_rows,
    );

    let mut done = vec![0; config.width];
    let duration = source.duration;

    for frame in source {
        let buffer = frame.buffer;
        let map = buffer.map_readable().unwrap();
        let indata = map.as_slice();

        let i = cmp::min(
            (config.width as f32 * (frame.pts.unwrap() / duration as f32)) as usize,
            config.width - 1,
        );

        let progress = 100.0 * frame.pts.unwrap() / duration as f32;
        print!("\rtimelens: {}% ", progress);
        stdout().flush().unwrap();

        {
            let timeline = timeline.buffer.get_mut().unwrap();
            let mut data = timeline.map_writable().unwrap();

            for y in 0..config.height {
                let mut b: usize = 0;
                let mut g: usize = 0;
                let mut r: usize = 0;

                for x in 0..config.tmp_width {
                    b += indata[config.tmp_width * y * 4 + 4 * x] as usize;
                    g += indata[config.tmp_width * y * 4 + 4 * x + 1] as usize;
                    r += indata[config.tmp_width * y * 4 + 4 * x + 2] as usize;
                }

                b /= config.tmp_width;
                g /= config.tmp_width;
                r /= config.tmp_width;

                data[config.width * y * 4 + i * 4] = b as u8;
                data[config.width * y * 4 + i * 4 + 1] = g as u8;
                data[config.width * y * 4 + i * 4 + 2] = r as u8;
                data[config.width * y * 4 + i * 4 + 3] = 255;
            }
        }

        {
            let thumbnails = thumbnails.buffer.get_mut().unwrap();
            let mut data = thumbnails.map_writable().unwrap();

            let tx = i % config.thumb_columns;
            let ty = i / config.thumb_columns;

            for x in 0..config.thumb_width {
                for y in 0..config.thumb_height {
                    let r = indata[y * config.thumb_width * 4 + 4 * x] as usize;
                    let g = indata[y * config.thumb_width * 4 + 4 * x + 1] as usize;
                    let b = indata[y * config.thumb_width * 4 + 4 * x + 2] as usize;

                    data[(config.thumb_columns * config.thumb_width * 4)
                             * (ty * config.thumb_height + y)
                             + (tx * config.thumb_width + x) * 4] = r as u8;
                    data[(config.thumb_columns * config.thumb_width * 4)
                             * (ty * config.thumb_height + y)
                             + (tx * config.thumb_width + x) * 4 + 1] = g as u8;
                    data[(config.thumb_columns * config.thumb_width * 4)
                             * (ty * config.thumb_height + y)
                             + (tx * config.thumb_width + x) * 4 + 2] = b as u8;
                    data[(config.thumb_columns * config.thumb_width * 4)
                             * (ty * config.thumb_height + y)
                             + (tx * config.thumb_width + x) * 4 + 3] = 255 as u8;
                }
            }
        }

        done[i as usize] += 1;

        if !done.contains(&0) {
            // we are done
            return (timeline, thumbnails);
        }
    }

    (timeline, thumbnails)
}

// write a WebVTT file pointing to the thumbnail locations
fn write_vtt(config: &Config, duration: f32) {
    let mseconds = (duration * 1_000_000.0) as i32;

    let mut f = File::create(&config.vtt_filename).unwrap();
    f.write_all(b"WEBVTT\n\n").unwrap();

    for i in 0..config.width {
        let from = mseconds / &(config.width as i32) * (i as i32);
        let to = mseconds / &(config.width as i32) * ((i as i32) + 1);

        let tx = i % &config.thumb_columns;
        let ty = i / &config.thumb_columns;

        let x = tx * &config.thumb_width;
        let y = ty * &config.thumb_height;

        let w = &config.thumb_width;
        let h = &config.thumb_height;

        let filename = Path::new(&config.thumbnails_filename)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap();

        write!(
            &mut f,
            "{} --> {}\n{}?xywh={},{},{},{}\n\n",
            timestamp(from),
            timestamp(to),
            filename,
            x,
            y,
            w,
            h
        ).unwrap();
    }
}
