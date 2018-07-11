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
use std::process;

mod frame;
mod source;

fn main() {
    // Parse the command line arguments
    let mut config = parse_config();

    // Create and initialize VideoSource
    let mut source = match source::VideoSource::new(
        &config.input_filename,
        config.thumbnail_height,
        config.width,
    ) {
        Ok(source) => source,
        Err(message) => error(&message),
    };

    // Derive thumbnail width and column count from the output width of the VideoSource
    config.thumbnail_width = source.width;
    let max_image_width = 5000;
    config.thumbnail_columns = max_image_width / config.thumbnail_width;

    // The hard part: generate the timeline and the thumbnail grid
    let (timeline, thumbnails) = generate_timeline_and_thumbnails(&config, &mut source);

    println!();

    if let Some(ref timeline_filename) = config.timeline_filename {
        // Write resulting timeline to a file
        timeline.write_to(&timeline_filename);
        println!("-> timeline witten to '{}'", timeline_filename);
    }

    if let Some(ref vtt_filename) = config.vtt_filename {
        // Write the VTT file
        write_vtt(&config, source.duration);
        println!("-> VTT written to '{}'", vtt_filename);
    }

    if let Some(ref thumbnails_filename) = config.thumbnails_filename {
        // Write resulting thumbnails to a file
        thumbnails.write_to(&thumbnails_filename);
        println!("-> thumbnail grid written to '{}'", thumbnails_filename);
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
    // Number of columns in the thumbnail grid
    thumbnail_columns: usize,

    // Name of the input file
    input_filename: String,
    // Name of the file the visual timeline will be written to
    timeline_filename: Option<String>,
    // Name of the file the thumbnail grid will be written to
    thumbnails_filename: Option<String>,
    // Name of the file the VTT file will be written to
    vtt_filename: Option<String>,
}

// Generate a Config from the command line arguments
fn parse_config() -> Config {
    let examples = vec![
        (
            "",
            "Generate a visual timeline called 'video.mp4.timeline.jpg' of default size.",
        ),
        (
            "--timeline timeline.jpg -w 1000 -h 500",
            "Override size and name of the timeline file.",
        ),
        (
            "--thumbnails thumbnails.vtt",
            "Generate thumbnail grids and a corresponding VTT file referencing their locations.",
        ),
    ];
    let examples = examples
        .iter()
        .map(|(cmd, desc)| format!("    timelens video.mp4 {}\n            {}\n", &cmd, desc))
        .collect::<Vec<String>>()
        .join("");

    let matches = app_from_crate!()
        .template("{bin} {version}\n{author}\n\n{about}\nUSAGE:\n    {usage}\n\nOPTIONS:\n{positionals}\n{unified}\nEXAMPLES:\n{after-help}")
        .setting(AppSettings::ArgRequiredElseHelp)
        .setting(AppSettings::NextLineHelp)
        .setting(AppSettings::UnifiedHelpMessage)
        .help_message("Print help information.")
        .version_message("Print version information.")
        .arg(
            Arg::with_name("input file")
                .value_name("INPUT_FILE")
                .help("Name of the input video file.")
                .index(1)
                .required(true),
        )
        .arg(
            Arg::with_name("width")
                .help(
                    "Width of the visual timeline in pixels [default: height*10, or 1000, if \
                     height is unspecified].",
                )
                .short("w")
                .long("width")
                .display_order(10)
                .takes_value(true)
                .value_name("NUM"),
        )
        .arg(
            Arg::with_name("height")
                .help("Height of the visual timeline in pixels [default: width/10].")
                .short("h")
                .long("height")
                .display_order(20)
                .takes_value(true)
                .value_name("NUM"),
        )
        .arg(
            Arg::with_name("timeline")
                .help(
                    "Create a visual timeline from the input file, which visualizes its color \
                     development. The result will be written to the specified file in JPEG format \
                     [default, if neither `--timeline` nor `--thumbnails` is used: \
                     INPUT_FILE.timeline.jpg].",
                )
                .long("timeline")
                .value_name("JPEG_FILE")
                .display_order(30)
                .takes_value(true),
        )
        .arg(
            Arg::with_name("thumbnails")
                .help(
                    "Extract thumbnails from the input file, in the form of one or several \
                     thumbnail grids. A VTT file referencing the thumbnails' positions will be \
                     written to the specified location. The number of thumbnails corresponds to \
                     the `--width` option, because the thumbnails are meant to be used together \
                     with the visual timeline.",
                )
                .long("thumbnails")
                .display_order(40)
                .value_name("VTT_FILE")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("thumbnail height")
                .help(
                    "Height of the individual thumbnails in the thumbnail grids. Their width is \
                     derived from the video's aspect ratio [default height: 90].",
                )
                .long("thumbnail-height")
                .short("H")
                .takes_value(true)
                .value_name("NUM")
                .display_order(50)
                .requires("thumbnails"),
        )
        // Hack to remove the autogenerated -V option, see
        // https://github.com/kbknapp/clap-rs/issues/1316.
        .arg(Arg::with_name("remove short version").short("V").hidden(true))
        .after_help(examples.as_str())
        .get_matches();

    // Set width and height, with multiple fallback cases
    let mut width: Option<usize> = None;
    let mut height: Option<usize> = None;

    if matches.is_present("width") {
        let width_string = matches.value_of("width").unwrap();

        width = if let Ok(width) = width_string.parse() {
            Some(width)
        } else {
            error("Width must be an integer");
        };
    }

    if matches.is_present("height") {
        let height_string = matches.value_of("height").unwrap();

        height = if let Ok(height) = height_string.parse() {
            Some(height)
        } else {
            error("Height must be an integer");
        };
    }

    if height.is_none() {
        if width.is_none() {
            width = Some(1000);
        }
        height = Some(width.unwrap() / 10);
    } else if width.is_none() {
        width = Some(height.unwrap() * 10);
    }

    if width.unwrap() < 16 {
        error("Timeline width must be at least 16");
    }

    if height.unwrap() < 16 {
        error("Timeline height must be at least 16");
    }

    if width.unwrap() > 65500 {
        error("Timeline width must be at most 65500");
    }

    if height.unwrap() > 65500 {
        error("Timeline height must be at most 65500");
    }

    //Set thumbnail height
    let thumbnail_height_string = matches.value_of("thumbnail height").unwrap_or("90");
    let thumbnail_height: usize = if let Ok(thumbnail_height) = thumbnail_height_string.parse() {
        thumbnail_height
    } else {
        error("Thumbnail height must be an integer");
    };

    if thumbnail_height < 16 {
        error("Thumbnail height must be at least 16");
    }

    if thumbnail_height > 65500 {
        error("Thumbnail height must be at most 65500");
    }

    let input_filename = matches.value_of("input file").unwrap();

    // Set timeline filename
    let timeline_filename = if matches.is_present("timeline") {
        let arg = String::from(matches.value_of("timeline").unwrap());
        let path = PathBuf::from(&arg);
        if path.extension().is_none() || path.extension().unwrap() != "jpg" {
            error("You must specify a .jpg file as an output for `--timeline`.");
        }
        Some(String::from(matches.value_of("timeline").unwrap()))
    } else if !matches.is_present("thumbnails") {
        Some(format!("{}.timelens.jpg", &input_filename))
    } else {
        None
    };

    // Set thumbnail-related filenames
    let vtt_filename = if matches.is_present("thumbnails") {
        let arg = String::from(matches.value_of("thumbnails").unwrap());
        let path = PathBuf::from(&arg);
        if path.extension().is_none() || path.extension().unwrap() != "vtt" {
            error("You must specify a .vtt file as an output for `--thumbnails`.");
        }
        Some(arg)
    } else {
        None
    };

    // Set VTT filename
    let thumbnails_filename = if vtt_filename.is_some() {
        Some(vtt_filename.clone().unwrap().replace(".vtt", "-01.jpg"))
    } else {
        None
    };

    check_for_collision(&input_filename, &timeline_filename);
    check_for_collision(&input_filename, &thumbnails_filename);
    check_for_collision(&input_filename, &vtt_filename);

    Config {
        width: width.unwrap(),
        height: height.unwrap(),

        thumbnail_width: 0,
        thumbnail_height: thumbnail_height,
        thumbnail_columns: 0,

        input_filename: String::from(input_filename),
        timeline_filename,
        thumbnails_filename,
        vtt_filename,
    }
}

fn error(message: &str) -> ! {
    eprintln!("{}: {}", "error".red().bold(), message);
    process::exit(1);
}

// The hard part: actually create timeline and thumbnails file
fn generate_timeline_and_thumbnails(
    config: &Config,
    source: &mut source::VideoSource,
) -> (frame::Frame, frame::Frame) {
    // Frame that will hold the visual timeline
    let mut timeline = frame::Frame::new(config.width, config.height);

    // Frame that will hold the thumbnail grid
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
            // Copy frame to the thumbnail grid
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
    let mseconds = (duration * 1_000.0) as i32;

    let thumbnails_filename = config.thumbnails_filename.clone().unwrap();
    let vtt_filename = config.vtt_filename.clone().unwrap();

    let mut f = match File::create(&vtt_filename) {
        Ok(file) => file,
        Err(e) => {
            error(&format!("Could not create '{}': {})", &vtt_filename, e));
        }
    };

    match f.write_all(b"WEBVTT\n\n") {
        Ok(_) => {}
        Err(e) => {
            error(&format!("Could not write to '{}': {})", &vtt_filename, e));
        }
    }

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

// Die if `new_opt` has a value that collides with `existing`
fn check_for_collision(existing: &str, new_opt: &Option<String>) {
    if let Some(new) = new_opt {
        let e = PathBuf::from(existing);
        let n = PathBuf::from(new);
        if e.exists()
            && n.exists()
            && fs::canonicalize(&e).unwrap() == fs::canonicalize(&n).unwrap()
        {
            error(&format!("Refusing to overwrite '{}'", existing));
        }
    }
}
