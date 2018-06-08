#[macro_use]
extern crate clap;
use clap::Arg;

extern crate glib;

extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;
use gst::prelude::*;

use std::cmp;

use std::io::stdout;
use std::io::Write;

use std::fs;
use std::fs::File;
use std::path::Path;
use std::path::PathBuf;

// Config objects are used to describe a single Timeline run
#[derive(Debug)]
struct Config {
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

// get the resolution of the input file
fn get_resolution(config: &Config) -> (i32, i32) {
    // generate file:// URI from an absolute filename
    let uri = format!(
        "file://{}",
        fs::canonicalize(&PathBuf::from(config.input_filename.as_str()))
            .unwrap()
            .to_str()
            .unwrap()
    );

    // set up a playbin element, which automatically select decoders
    let playbin = gst::ElementFactory::make("playbin", None).unwrap();
    playbin.set_property("uri", &uri).unwrap();

    // we don't actually want any output, so we connect the playbin to a fakesink
    let fakesink = gst::ElementFactory::make("fakesink", None).unwrap();
    playbin.set_property("video-sink", &fakesink).unwrap();

    // create a pipeline and add the playbin to it
    let pipeline = gst::Pipeline::new(None);
    pipeline.add(&playbin).unwrap();

    // set pipeline state to "paused" to start pad negotiation
    pipeline
        .set_state(gst::State::Paused)
        .into_result()
        .unwrap();
    pipeline.get_state(10 * gst::SECOND);

    // get the sinkpad of the first video stream
    let pad = playbin
        .emit("get-video-pad", &[&0])
        .unwrap()
        .unwrap()
        .get::<gst::Pad>()
        .unwrap();

    // and retrieve width and height from its caps
    let caps = pad.get_current_caps().unwrap();
    let width = caps.get_structure(0)
        .unwrap()
        .get_value("width")
        .unwrap()
        .get::<i32>()
        .unwrap();
    let height = caps.get_structure(0)
        .unwrap()
        .get_value("height")
        .unwrap()
        .get::<i32>()
        .unwrap();

    pipeline.set_state(gst::State::Null).into_result().unwrap();

    (width, height)
}

// build a pipeline that decodes the video to BGRx at 1 FPS, scales the frames to thumbnail size,
// and hands it to an Appsink
fn build_input_pipeline(config: &Config) -> (gst::Pipeline, gst::Element, gst_app::AppSink) {
    let uri = format!(
        "file://{}",
        fs::canonicalize(&PathBuf::from(config.input_filename.as_str()))
            .unwrap()
            .to_str()
            .unwrap()
    );

    let src = gst::ElementFactory::make("uridecodebin", None).unwrap();
    src.set_property("uri", &uri).unwrap();

    let videoconvert = gst::ElementFactory::make("videoconvert", None).unwrap();
    let videorate = gst::ElementFactory::make("videorate", None).unwrap();
    let videoscale = gst::ElementFactory::make("videoscale", None).unwrap();
    // scale frames exactly to the desired size, don't add borders
    videoscale.set_property("add-borders", &false).unwrap();

    let capsfilter = gst::ElementFactory::make("capsfilter", None).unwrap();
    capsfilter
        .set_property(
            "caps",
            &gst::Caps::new_simple(
                "video/x-raw",
                &[
                    ("format", &"BGRx"),
                    ("framerate", &gst::Fraction::new(1, 1)),
                    ("width", &(config.thumb_width as i32)),
                    ("height", &(config.thumb_height as i32)),
                ],
            ),
        )
        .unwrap();

    let sink = gst::ElementFactory::make("appsink", None).unwrap();

    let pipeline = gst::Pipeline::new(None);

    pipeline
        .add_many(&[
            &src,
            &videoconvert,
            &videorate,
            &videoscale,
            &capsfilter,
            &sink,
        ])
        .unwrap();

    gst::Element::link_many(&[&videoconvert, &videorate, &videoscale, &capsfilter, &sink]).unwrap();

    let appsink = sink.clone()
        .dynamic_cast::<gst_app::AppSink>()
        .expect("Sink element is expected to be an appsink!");
    // go as fast as possible :)
    appsink.set_property("sync", &false).unwrap();

    // when a new source pad opens on the decodebin, connect it to the videoconvert element.
    // this code is required because media files might contain no (or many) video strems, this is
    // not known before the pipeline is started.
    let convert_clone = videoconvert.clone();
    src.connect_pad_added(move |_, src_pad| {
        let convert = &convert_clone;

        let sink_pad = convert
            .get_static_pad("sink")
            .expect("Failed to get static sink pad from convert");

        if sink_pad.is_linked() {
            // we are already linked. ignoring.
            return;
        }

        let new_pad_caps = src_pad
            .get_current_caps()
            .expect("Failed to get caps of new pad.");
        let new_pad_struct = new_pad_caps
            .get_structure(0)
            .expect("Failed to get first structure of caps.");
        let new_pad_type = new_pad_struct.get_name();

        let is_audio = new_pad_type.starts_with("video/x-raw");
        if !is_audio {
            println!(
                "It has type {} which is not raw video. Ignoring.",
                new_pad_type
            );
            return;
        }

        let ret = src_pad.link(&sink_pad);
        if ret != gst::PadLinkReturn::Ok {
            println!("Type is {} but link failed.", new_pad_type);
        }
    });

    (pipeline, capsfilter, appsink)
}

// build a pipeline that writes the first frame pushed into the Appsrc to a JPEG file
fn build_output_pipeline(
    width: i32,
    height: i32,
    filename: &String,
) -> (gst::Pipeline, gst_app::AppSrc) {
    let src = gst::ElementFactory::make("appsrc", None).unwrap();

    let capsfilter = gst::ElementFactory::make("capsfilter", None).unwrap();
    capsfilter
        .set_property(
            "caps",
            &gst::Caps::new_simple(
                "video/x-raw",
                &[
                    ("format", &"BGRx"),
                    ("framerate", &gst::Fraction::new(1, 1)),
                    ("width", &width),
                    ("height", &height),
                ],
            ),
        )
        .unwrap();

    let jpegenc = gst::ElementFactory::make("jpegenc", None).unwrap();
    let filesink = gst::ElementFactory::make("filesink", None).unwrap();
    filesink.set_property("location", &filename).unwrap();

    let output_pipeline = gst::Pipeline::new(None);
    output_pipeline
        .add_many(&[&src, &capsfilter, &jpegenc, &filesink])
        .unwrap();
    gst::Element::link_many(&[&src, &capsfilter, &jpegenc, &filesink]).unwrap();

    let appsrc = src.clone()
        .dynamic_cast::<gst_app::AppSrc>()
        .expect("Sink element is expected to be an appsrc!");
    appsrc.set_property_format(gst::Format::Time);
    appsrc.set_property_block(true);

    (output_pipeline, appsrc)
}

// the hard part: actually create timeline and thumbnails file
fn generate_timeline_and_thumbnails(
    config: &Config,
    input_pipeline: &gst::Pipeline,
    appsink: &gst_app::AppSink,
    duration: &gst::ClockTime,
) -> (gst::Buffer, gst::Buffer) {
    let mut timeline = gst::Buffer::with_size(config.width * config.height * 4).unwrap();

    let thumb_rows = config.width / config.thumb_columns + 1;
    let mut thumbnails = gst::Buffer::with_size(
        config.thumb_width * config.thumb_columns * config.thumb_height * thumb_rows * 4,
    ).unwrap();

    let mut done = vec![0; config.width];

    let mut next_column = 0;

    loop {
        let sample = match appsink.pull_sample() {
            None => {
                // we are probably at the end
                return (timeline, thumbnails);
            }
            Some(sample) => sample,
        };

        let buffer = sample.get_buffer().unwrap();
        let map = buffer.map_readable().unwrap();
        let indata = map.as_slice();

        let pts: gst::ClockTime = buffer.get_pts();
        let i = cmp::min(
            config.width * (pts.nseconds().unwrap() as usize)
                / (duration.nseconds().unwrap() as usize),
            config.width - 1,
        );

        let progress = 100 * pts.nseconds().unwrap() / duration.nseconds().unwrap();
        print!("\rtimelens: {}% ", progress);
        stdout().flush().unwrap();

        {
            let timeline = timeline.get_mut().unwrap();
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
            let thumbnails = thumbnails.get_mut().unwrap();
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

        if config.seek_mode {
            next_column += 1;

            let j = (duration.nseconds().unwrap() as usize) / config.width * next_column;

            input_pipeline
                .seek_simple(
                    gst::SeekFlags::FLUSH, // | gst::SeekFlags::KEY_UNIT,
                    (j as u64) * gst::NSECOND,
                )
                .unwrap();
        }
    }
}

// push a buffer into an output pipeline and wait for EOS
fn write_result(
    timeline: &gst::Buffer,
    output_pipeline: &gst::Pipeline,
    output_src: &gst_app::AppSrc,
) {
    output_pipeline
        .set_state(gst::State::Playing)
        .into_result()
        .unwrap();
    output_src
        .push_buffer(timeline.copy_deep().unwrap())
        .into_result()
        .unwrap();
    output_src.end_of_stream().into_result().unwrap();
}

// write a WebVTT file pointing to the thumbnail locations
fn write_vtt(config: &Config, duration: &gst::ClockTime) {
    let mseconds = duration.mseconds().unwrap() as i32;

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

fn main() {
    // parse the command line arguments
    let mut config = parse_config();

    // initialize GStreamer
    gst::init().unwrap();

    // calculate desired thumbnail width
    let (width, height) = get_resolution(&config);
    let aspect_ratio = (1000 * width / height) as usize;
    config.thumb_width = config.thumb_height * aspect_ratio / 1000;
    config.tmp_width = config.thumb_height * aspect_ratio / 1000;

    // build input and output pipelines
    let (input_pipeline, capsfilter, appsink) = build_input_pipeline(&config);
    let (output_pipeline, output_src) = build_output_pipeline(
        config.width as i32,
        config.height as i32,
        &config.timeline_filename,
    );
    let (output_pipeline2, output_src2) = build_output_pipeline(
        ((config.thumb_width * config.thumb_columns) as i32),
        ((config.thumb_height * (config.width / config.thumb_columns + 1)) as i32),
        &config.thumbnails_filename,
    );

    // set the input pipeline to paused to fill the buffers
    input_pipeline
        .set_state(gst::State::Paused)
        .into_result()
        .unwrap();
    input_pipeline.get_state(10 * gst::SECOND);

    // if we don't seek, start playing
    if !config.seek_mode {
        input_pipeline
            .set_state(gst::State::Playing)
            .into_result()
            .unwrap();
    }

    let duration: gst::ClockTime = input_pipeline.query_duration().unwrap();
    let fps = gst::Fraction::new(config.width as i32, duration.seconds().unwrap() as i32);

    capsfilter
        .set_property(
            "caps",
            &gst::Caps::new_simple(
                "video/x-raw",
                &[
                    ("format", &"BGRx"),
                    ("framerate", &fps),
                    ("width", &(config.thumb_width as i32)),
                    ("height", &(config.thumb_height as i32)),
                ],
            ),
        )
        .unwrap();

    let main_loop = glib::MainLoop::new(None, false);

    for pipeline in &[&input_pipeline, &output_pipeline] {
        let bus = pipeline.get_bus().unwrap();
        bus.connect_message(move |_, msg| match msg.view() {
            gst::MessageView::Eos(_) => {}
            gst::MessageView::Error(err) => {
                eprintln!(
                    "Error received from element {:?}: {}",
                    err.get_src().map(|s| s.get_path_string()),
                    err.get_error()
                );
                eprintln!("Debugging information: {:?}", err.get_debug());
            }
            _ => {}
        });
        bus.add_signal_watch();
    }

    for pipeline in &[&output_pipeline2] {
        let bus = pipeline.get_bus().unwrap();
        let main_loop_clone = main_loop.clone();
        bus.connect_message(move |_, msg| match msg.view() {
            gst::MessageView::Eos(_) => {
                main_loop_clone.quit();
            }
            gst::MessageView::Error(err) => {
                eprintln!(
                    "Error received from element {:?}: {}",
                    err.get_src().map(|s| s.get_path_string()),
                    err.get_error()
                );
                eprintln!("Debugging information: {:?}", err.get_debug());
            }
            _ => {}
        });
        bus.add_signal_watch();
    }

    let (timeline, thumbnails) =
        generate_timeline_and_thumbnails(&config, &input_pipeline, &appsink, &duration);

    write_result(&timeline, &output_pipeline, &output_src);
    write_result(&thumbnails, &output_pipeline2, &output_src2);

    main_loop.run();

    println!("-> '{}'", config.timeline_filename);
    println!("-> '{}'", config.thumbnails_filename);
    write_vtt(&config, &duration);
    println!("-> '{}'", config.vtt_filename);

    input_pipeline
        .set_state(gst::State::Null)
        .into_result()
        .unwrap();
    output_pipeline
        .set_state(gst::State::Null)
        .into_result()
        .unwrap();
    output_pipeline2
        .set_state(gst::State::Null)
        .into_result()
        .unwrap();
}
