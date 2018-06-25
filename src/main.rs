#[macro_use]
extern crate clap;
extern crate colored;

use clap::AppSettings;
use clap::Arg;
use colored::*;
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
    // Parse the command line arguments
    let mut config = parse_config();

    // Create and initialize VideoSource
    let mut source = source::VideoSource::new(
        &config.input_filename,
        config.thumbnail_height,
        config.width,
    );

    // Derive thumbnail width and column count from the output width of the VideoSource
    config.thumbnail_width = source.width;
    let max_image_width = 5000;
    config.thumbnail_columns = max_image_width / config.thumbnail_width;

    // The hard part: generate the timeline and the thumbnail sheet
    let (timeline, thumbnails) = generate_timeline_and_thumbnails(&config, &mut source);

    println!();

    if let Some(ref timeline_filename) = config.timeline_filename {
        // Write resulting timeline to a file
        timeline.write_to(&timeline_filename);
        println!("-> timeline witten to '{}'", timeline_filename);
    }

    if let Some(ref thumbnails_filename) = config.thumbnails_filename {
        // Write resulting thumbnails to a file
        thumbnails.write_to(&thumbnails_filename);
        println!("-> thumbnails written to '{}'", thumbnails_filename);
    }

    if let Some(ref vtt_filename) = config.vtt_filename {
        // Write the VTT file
        write_vtt(&config, source.duration);
        println!("-> VTT written to '{}'", vtt_filename);
    }
}

// Config objects are used to describe a single Timeline run
pub struct Config {
    // Width of visual timeline
    width: usize,
    // Height of visual timeline
    height: usize,

    // Width of single thumbnail
    thumbnail_width: usize,
    // Height of single thumbnail
    thumbnail_height: usize,
    // Number of columns in the thumbnail sheet
    thumbnail_columns: usize,

    // Name of the input file
    input_filename: String,
    // Name of the file the visual timeline will be written to
    timeline_filename: Option<String>,
    // Name of the file the thumbnail sheet will be written to
    thumbnails_filename: Option<String>,
    // Name of the file the VTT file will be written to
    vtt_filename: Option<String>,
}

// Generate a Config from the command line arguments
fn parse_config() -> Config {
    let examples = vec![
        (
            "video.mp4",
            "Generate video.mp4.timeline.jpg of default size",
        ),
        (
            "video.mp4 -w 1000 -h 500 --timeline output.jpg",
            "Override size and name of the timeline file",
        ),
        (
            "video.mp4 --thumbnails thumbnails.jpg --vtt thumbnails.vtt",
            "Generate a thumbnail sheet and a corresponding VTT file",
        ),
    ];

    let examples_string = examples
        .iter()
        .map(|(cmd, desc)| format!("    {}\n            {}\n", cmd.green(), desc))
        .collect::<Vec<String>>()
        .join("");

    let examples_section = format!("{}:\n{}", "EXAMPLES".yellow(), &examples_string);

    let matches = app_from_crate!()
        .setting(AppSettings::ArgRequiredElseHelp)
        .setting(AppSettings::ColoredHelp)
        .setting(AppSettings::NextLineHelp)
        .arg(
            Arg::with_name("input file")
                .help("Name of the video file")
                .index(1)
                .required(true),
        )
        .arg(
            Arg::with_name("width")
                .help("Set width of the visual timeline [default: height*10, or 1000, if height is unspecified]")
                .short("w")
                .long("width")
                .display_order(0)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("height")
                .help("Set height of the visual timeline [default: width/10]")
                .short("h")
                .long("height")
                .display_order(1)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("thumbnail height")
                .help("Set height of the individual thumbnails")
                .long("thumbnail-height")
                .takes_value(true)
                .value_name("height")
        )
        .arg(
            Arg::with_name("timeline")
                .help("Name of timeline output file. If no output file is specified at all, write a timeline to 'INPUT_FILE.timeline.jpg' by default")
                .long("timeline")
                .value_name("filename")
                .display_order(10)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("thumbnails")
                .help("Name of thumbnails output file")
                .long("thumbnails")
                .value_name("filename")
                .display_order(11)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("vtt")
                .help("Name of VTT output file, which contains information about the position of the individual thumbnails in the thumbnails output file. Requires --thumbnails")
                .long("vtt")
                .value_name("filename")
                .requires("thumbnails")
                .display_order(12)
                .takes_value(true),
        )
        .after_help(examples_section.as_str())
        .get_matches();

    let mut width: Option<usize> = None;
    let mut height: Option<usize> = None;

    if matches.is_present("width") {
        let width_string = matches.value_of("width").unwrap();
        width = Some(width_string.parse().expect("Invalid width"));
    }

    if matches.is_present("height") {
        let height_string = matches.value_of("height").unwrap();
        height = Some(height_string.parse().expect("Invalid height"));
    }

    if height.is_none() {
        if width.is_none() {
            width = Some(1000);
        }
        height = Some(width.unwrap() / 10);
    } else {
        if width.is_none() {
            width = Some(height.unwrap() * 10);
        }
    }

    let thumbnail_height_string = matches.value_of("thumbnail height").unwrap_or("90");
    let thumbnail_height: usize = thumbnail_height_string
        .parse()
        .expect("Invalid thumbnail height");

    let input_filename = matches.value_of("input file").unwrap();

    // Set default timeline filename
    let fallback_output = format!("{}.timeline.jpg", &input_filename);
    let timeline_filename = if !matches.is_present("thumbnails") {
        Some(String::from(
            matches.value_of("timeline").unwrap_or(&fallback_output),
        ))
    } else {
        match matches.value_of("timeline") {
            Some(timeline) => Some(String::from(timeline)),
            None => None,
        }
    };

    let thumbnails_filename = if matches.is_present("thumbnails") {
        Some(String::from(matches.value_of("thumbnails").unwrap()))
    } else {
        None
    };

    let vtt_filename = if matches.is_present("vtt") {
        Some(String::from(matches.value_of("vtt").unwrap()))
    } else {
        None
    };

    check_for_collision(&input_filename, &timeline_filename);
    check_for_collision(&input_filename, &thumbnails_filename);
    check_for_collision(&input_filename, &timeline_filename);

    Config {
        width: width.unwrap(),
        height: height.unwrap(),

        thumbnail_width: 0,
        //thumbnail_height: height.unwrap(), //thumbnail_height,
        thumbnail_height: thumbnail_height,
        thumbnail_columns: 0,

        input_filename: String::from(input_filename),
        timeline_filename,
        thumbnails_filename,
        vtt_filename,
    }
}

// The hard part: actually create timeline and thumbnails file
fn generate_timeline_and_thumbnails(
    config: &Config,
    source: &mut source::VideoSource,
) -> (frame::Frame, frame::Frame) {
    // Frame that will hold the visual timeline
    let mut timeline = frame::Frame::new(config.width, config.height);

    // Frame that will hold the thumbnail sheet
    let thumbnail_rows = config.width / config.thumbnail_columns + 1;
    let mut thumbnails = frame::Frame::new(
        config.thumbnail_width * config.thumbnail_columns,
        config.thumbnail_height * thumbnail_rows,
    );

    // Keep track of which columns are already done
    let mut done = vec![0; config.width];

    // Remember duration before moving `source`
    let duration = source.duration;

    // Iterate over the frames from the source (which arrive in any order)
    for frame in source {
        // Calculate which column this frame belongs to
        let i = cmp::min(
            (config.width as f32 * (frame.pts.unwrap() / duration as f32)) as usize,
            config.width - 1,
        );

        if config.timeline_filename.is_some() {
            // Scale frame to 1 pixel width and copy into the timeline
            let column = frame.scale(1, config.height);
            timeline.copy(&column, i, 0);
        }

        if config.thumbnails_filename.is_some() {
            // Copy frame to the thumbnail sheet
            let tx = i % config.thumbnail_columns;
            let ty = i / config.thumbnail_columns;
            thumbnails.copy(
                &frame,
                tx * config.thumbnail_width,
                ty * config.thumbnail_height,
            );
        }

        done[i as usize] += 1;

        // Calculate and report progress
        let columns_done = done.iter().filter(|&n| *n > 0).count();
        let progress = 100.0 * columns_done as f32 / config.width as f32;
        print!("\rtimelens: {:.1}% ", progress);
        stdout().flush().unwrap();
    }

    (timeline, thumbnails)
}

// Convert milliseconds to a WebVTT timestamp (which has the format "(HH:)MM:SS.mmmm")
fn timestamp(mseconds_total: i32) -> String {
    let minutes = mseconds_total / (1000 * 60);
    let seconds = (mseconds_total - 1000 * 60 * minutes) / 1000;
    let mseconds = mseconds_total - 1000 * (seconds + 60 * minutes);
    format!("{:02}:{:02}.{:03}", minutes, seconds, mseconds)
}

// Write a WebVTT file pointing to the thumbnail locations
fn write_vtt(config: &Config, duration: f32) {
    let mseconds = (duration * 1_000_000.0) as i32;

    let thumbnails_filename = config.thumbnails_filename.clone().unwrap();
    let vtt_filename = config.vtt_filename.clone().unwrap();

    let mut f = File::create(&vtt_filename).unwrap();
    f.write_all(b"WEBVTT\n\n").unwrap();

    for i in 0..config.width {
        let from = mseconds / (config.width as i32) * (i as i32);
        let to = mseconds / (config.width as i32) * ((i as i32) + 1);

        let tx = i % config.thumbnail_columns;
        let ty = i / config.thumbnail_columns;

        let x = tx * config.thumbnail_width;
        let y = ty * config.thumbnail_height;

        let w = config.thumbnail_width;
        let h = config.thumbnail_height;

        let filename = Path::new(&thumbnails_filename)
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

// Panic if `new_opt` has a value that collides with `existing`
fn check_for_collision(existing: &str, new_opt: &Option<String>) {
    if let Some(new) = new_opt {
        let e = PathBuf::from(existing);
        let n = PathBuf::from(new);
        if e.exists()
            && n.exists()
            && fs::canonicalize(&e).unwrap() == fs::canonicalize(&n).unwrap()
        {
            panic!("Refusing to overwrite '{}'", existing);
        }
    }
}
