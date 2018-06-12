#[macro_use]
extern crate clap;

use clap::Arg;
use std::cmp;
use std::fs;
use std::fs::File;
use std::io::stdout;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

mod frame;
mod source;

fn main() {
    // parse the command line arguments
    let mut config = parse_config();

    // create and initialize VideoSource
    let source =
        source::VideoSource::new(&config.input_filename, config.thumb_height, config.width);

    // derive thumbnail width from the output width of the VideoSource
    config.thumb_width = source.width;

    // remember the video duration
    let duration = source.duration;

    // the hard part: generate the timeline and the thumbnail sheet
    let (timeline, thumbnails) = generate_timeline_and_thumbnails(&config, source);

    // write resulting images to JPEG files
    println!("");
    timeline.write_to(&config.timeline_filename);
    println!("-> '{}'", config.timeline_filename);
    thumbnails.write_to(&config.thumbnails_filename);
    println!("-> '{}'", config.thumbnails_filename);

    // write the VTT file
    write_vtt(&config, duration);
    println!("-> '{}'", config.vtt_filename);
}

// Config objects are used to describe a single Timeline run
pub struct Config {
    // width of visual timeline
    width: usize,
    // height of visual timeline
    height: usize,

    // width of single thumbnail
    thumb_width: usize,
    // height of single thumbnail
    thumb_height: usize,
    // number of columns in the thumbnail sheet // TODO: remove
    thumb_columns: usize,

    // name of the input file
    input_filename: String,
    // name of the file the visual timeline will be written to
    timeline_filename: String,
    // name of the file the thumbnail sheet will be written to
    thumbnails_filename: String,
    // name of the file the VTT file will be written to
    vtt_filename: String,
}

// generate a Config from the command line arguments
fn parse_config() -> Config {
    let matches = app_from_crate!()
        .arg(
            Arg::with_name("input file")
                .help("Input file")
                .index(1)
                .required(true),
        )
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
        .get_matches();

    // default width is 1000
    let width_string = matches.value_of("width").unwrap_or("1000");
    let width: usize = width_string.parse().expect("Invalid width");

    // default height is 100
    let height_string = matches.value_of("height").unwrap_or("100");
    let height: usize = height_string.parse().expect("Invalid height");

    let input_filename = matches.value_of("input file").unwrap();

    // default output filenames are extensions of the input filename
    let fallback_output = format!("{}.timeline.jpg", &input_filename);
    let timeline_filename = matches.value_of("timeline").unwrap_or(&fallback_output);
    //check_for_collision(&input_filename, &timeline_filename);

    let fallback_output2 = format!("{}.thumbnails.jpg", &input_filename);
    let thumbnails_filename = matches.value_of("thumbnails").unwrap_or(&fallback_output2);
    //check_for_collision(&input_filename, &thumbnails_filename);

    let fallback_output3 = format!("{}.thumbnails.vtt", &input_filename);
    let vtt_filename = matches.value_of("vtt").unwrap_or(&fallback_output3);
    //check_for_collision(&input_filename, &vtt_filename);

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
    }
}

// the hard part: actually create timeline and thumbnails file
fn generate_timeline_and_thumbnails(
    config: &Config,
    source: source::VideoSource,
) -> (frame::Frame, frame::Frame) {
    // frame that will hold the visual timeline
    let mut timeline = frame::Frame::new(config.width, config.height);

    // frame that will hold the thumbnail sheet
    let thumb_rows = config.width / config.thumb_columns + 1;
    let mut thumbnails = frame::Frame::new(
        config.thumb_width * config.thumb_columns,
        config.thumb_height * thumb_rows,
    );

    // keep track of which columns are already done
    let mut done = vec![0; config.width];

    // remember duration before moving `source`
    let duration = source.duration;

    // iterate over the frames from the source (which arrive in any order)
    for frame in source {
        // calculate which column this frame belongs to
        let i = cmp::min(
            (config.width as f32 * (frame.pts.unwrap() / duration as f32)) as usize,
            config.width - 1,
        );

        // scale frame to 1 pixel width and copy into the timeline
        let column = frame.scale(1, frame.height);
        timeline.copy(&column, i, 0);

        // copy frame to the thumbnail sheet
        let tx = i % config.thumb_columns;
        let ty = i / config.thumb_columns;
        thumbnails.copy(&frame, tx * config.thumb_width, ty * config.thumb_height);

        done[i as usize] += 1;

        // calculate and report progress
        let columns_done = done.iter().filter(|&n| *n > 0).count();
        let progress = 100.0 * columns_done as f32 / config.width as f32;
        print!("\rtimelens: {:.1}% ", progress);
        stdout().flush().unwrap();

        if !done.contains(&0) {
            // we are done
            return (timeline, thumbnails);
        }
    }

    (timeline, thumbnails)
}

// convert milliseconds to a WebVTT timestamp (which has the format "(HH:)MM:SS.mmmm")
fn timestamp(mseconds_total: i32) -> String {
    let minutes = mseconds_total / (1000 * 60);
    let seconds = (mseconds_total - 1000 * 60 * minutes) / 1000;
    let mseconds = mseconds_total - 1000 * (seconds + 60 * minutes);
    format!("{:02}:{:02}.{:03}", minutes, seconds, mseconds)
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

fn check_for_collision(existing: &str, new: &str) {
    if fs::canonicalize(&PathBuf::from(existing)).unwrap()
        == fs::canonicalize(&PathBuf::from(new)).unwrap()
    {
        panic!("Refusing to overwrite '{}'", existing);
    }
}
