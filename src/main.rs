extern crate clap;
use clap::{App, Arg};
extern crate glib;
extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;
use gst::prelude::*;
use std::cmp;

use std::io::Write;
use std::io::stdout;

#[derive(Debug)]
struct Config {
    width: u64,
    height: u64,
    input_filename: String,
    output_filename: String,
    tmp_width: u64,
}

fn parse_config() -> Config {
    let matches = App::new("nordlicht")
        .author("Sebastian Morr <sebastian@morr.cc>")
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
            Arg::with_name("output")
                .help("Name of output file")
                .short("o")
                .long("output")
                .takes_value(true),
        )
        .get_matches();

    let width_string = matches.value_of("width").unwrap_or("1920");
    let width: u64 = width_string.parse().expect("Invalid width");

    let height_string = matches.value_of("height").unwrap_or("192");
    let height: u64 = height_string.parse().expect("Invalid height");

    let input_filename = matches
        .value_of("input")
        .unwrap_or("/home/seb/library/movies/Blender Shorts/big-buck-bunny.avi");

    let fallback_output = format!("{}.nordlicht.jpg", &input_filename);
    let output_filename = matches.value_of("output").unwrap_or(&fallback_output);

    Config {
        width: width,
        height: height,
        input_filename: String::from(input_filename),
        output_filename: String::from(output_filename),
        tmp_width: 100,
    }
}

fn build_input_pipeline(config: &Config) -> (gst::Pipeline, gst::Element, gst_app::AppSink) {
    let uri = format!("file://{}", config.input_filename);

    let src = gst::ElementFactory::make("uridecodebin", None).unwrap();
    src.set_property("uri", &uri).unwrap();

    let videorate = gst::ElementFactory::make("videorate", None).unwrap();
    let videoscale = gst::ElementFactory::make("videoscale", None).unwrap();
    videoscale.set_property("add-borders", &false).unwrap();
    let videoconvert = gst::ElementFactory::make("videoconvert", None).unwrap();

    let capsfilter = gst::ElementFactory::make("capsfilter", None).unwrap();
    capsfilter
        .set_property(
            "caps",
            &gst::Caps::new_simple(
                "video/x-raw",
                &[
                    ("format", &"BGRx"),
                    ("framerate", &gst::Fraction::new(1, 1)),
                    ("width", &(config.tmp_width as i32)),
                    ("height", &(config.height as i32)),
                ],
            ),
        )
        .unwrap();

    let sink = gst::ElementFactory::make("appsink", None).unwrap();

    let pipeline = gst::Pipeline::new(None);

    pipeline
        .add_many(&[
            &src,
            &videorate,
            &videoscale,
            &videoconvert,
            &capsfilter,
            &sink,
        ])
        .unwrap();

    gst::Element::link_many(&[&videorate, &videoscale, &videoconvert, &capsfilter, &sink]).unwrap();

    let appsink = sink.clone()
        .dynamic_cast::<gst_app::AppSink>()
        .expect("Sink element is expected to be an appsink!");
    appsink.set_property("sync", &false).unwrap();

    let pipeline_clone = pipeline.clone();
    let convert_clone = videorate.clone();
    src.connect_pad_added(move |_, src_pad| {
        let pipeline = &pipeline_clone;
        let convert = &convert_clone;

        println!(
            "Received new pad {} from {}",
            src_pad.get_name(),
            pipeline.get_name()
        );

        let sink_pad = convert
            .get_static_pad("sink")
            .expect("Failed to get static sink pad from convert");
        if sink_pad.is_linked() {
            println!("We are already linked. Ignoring.");
            return;
        }

        //gst::debug_bin_to_dot_file(&pipeline_clone, gst::DebugGraphDetails::ALL, "output");

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
        } else {
            println!("Link succeeded (type {}).", new_pad_type);
        }
    });

    (pipeline, capsfilter, appsink)
}

fn build_output_pipeline(config: &Config) -> (gst::Pipeline, gst_app::AppSrc) {
    let output_pipeline = gst::Pipeline::new(None);

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
                    ("width", &(config.width as i32)),
                    ("height", &(config.height as i32)),
                ],
            ),
        )
        .unwrap();

    let jpegenc = gst::ElementFactory::make("jpegenc", None).unwrap();
    let filesink = gst::ElementFactory::make("filesink", None).unwrap();
    filesink
        .set_property("location", &config.output_filename)
        .unwrap();
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

fn build_preview_pipeline(config: &Config) -> (gst::Pipeline, gst_app::AppSrc) {
    let preview_pipeline = gst::Pipeline::new(None);

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
                    ("width", &(config.width as i32)),
                    ("height", &(config.height as i32)),
                ],
            ),
        )
        .unwrap();
    let videoconvert = gst::ElementFactory::make("videoconvert", None).unwrap();

    let sink = gst::ElementFactory::make("autovideosink", None).unwrap();
    sink.set_property("sync", &false).unwrap();

    preview_pipeline
        .add_many(&[&src, &capsfilter, &videoconvert, &sink])
        .unwrap();
    gst::Element::link_many(&[&src, &capsfilter, &videoconvert, &sink]).unwrap();

    let appsrc = src.clone()
        .dynamic_cast::<gst_app::AppSrc>()
        .expect("Sink element is expected to be an appsrc!");
    appsrc.set_property_format(gst::Format::Time);
    appsrc.set_property_block(true);

    match preview_pipeline.set_state(gst::State::Playing) {
        gst::StateChangeReturn::Success => println!("success"),
        gst::StateChangeReturn::Failure => println!("failure"),
        gst::StateChangeReturn::Async => println!("async"),
        gst::StateChangeReturn::NoPreroll => println!("nopreroll"),
        _ => println!("other"),
    }

    (preview_pipeline, appsrc)
}

fn main() {
    let config = parse_config();
    println!("{:#?}", config);

    // Initialize GStreamer
    gst::init().unwrap();

    let (input_pipeline, capsfilter, appsink) = build_input_pipeline(&config);
    let (output_pipeline, output_src) = build_output_pipeline(&config);
    let (preview_pipeline, preview_src) = build_preview_pipeline(&config);

    input_pipeline
        .set_state(gst::State::Playing)
        .into_result()
        .unwrap();

    input_pipeline.get_state(10 * gst::SECOND);

    let duration: gst::ClockTime = input_pipeline.query_duration().unwrap();
    let fps = gst::Fraction::new(config.width as i32, duration.seconds().unwrap() as i32);
    println!("fps: {}", fps);

    for pipeline in [&input_pipeline, &output_pipeline, &preview_pipeline].iter() {
        let bus = pipeline.get_bus().unwrap();
        bus.connect_message(move |_, msg| match msg.view() {
            gst::MessageView::Error(err) => {
                eprintln!(
                    "Error received from element {:?}: {}",
                    err.get_src().map(|s| s.get_path_string()),
                    err.get_error()
                    );
                eprintln!("Debugging information: {:?}", err.get_debug());
            }
            _ => (),
        });
        bus.add_signal_watch();
    }

    capsfilter
        .set_property(
            "caps",
            &gst::Caps::new_simple(
                "video/x-raw",
                &[
                    ("format", &"BGRx"),
                    ("framerate", &fps),
                    ("width", &(config.tmp_width as i32)),
                    ("height", &(config.height as i32)),
                ],
            ),
        )
        .unwrap();

    let mut outbuffer =
        gst::Buffer::with_size((config.width * config.height * 4) as usize).unwrap();

    loop {
        let sample = match appsink.pull_sample() {
            None => {
                output_pipeline.set_state(gst::State::Playing).into_result().unwrap();
                output_src.push_buffer(outbuffer.copy_deep().unwrap()).into_result().unwrap();
                output_src.end_of_stream().into_result().unwrap();
                break;
            }
            Some(sample) => sample,
        };

        let buffer = sample.get_buffer().unwrap();
        let map = buffer.map_readable().unwrap();
        let indata = map.as_slice();

        let pts: gst::ClockTime = buffer.get_pts();
        let i = cmp::min(
            (config.width * pts.nseconds().unwrap() / duration.nseconds().unwrap()),
            config.width - 1,
        );

        let progress = 100*pts.nseconds().unwrap() / duration.nseconds().unwrap();
        print!("\rnordlicht: {}%", progress);
        stdout().flush().unwrap();

        {
            let outbuffer = outbuffer.get_mut().unwrap();

            let mut data = outbuffer.map_writable().unwrap();

            for y in 0..config.height {
                let mut b: u64 = 0;
                let mut g: u64 = 0;
                let mut r: u64 = 0;

                for x in 0..config.tmp_width {
                    b += indata[(config.tmp_width * y * 4 + 4 * x + 0) as usize] as u64;
                    g += indata[(config.tmp_width * y * 4 + 4 * x + 1) as usize] as u64;
                    r += indata[(config.tmp_width * y * 4 + 4 * x + 2) as usize] as u64;
                }

                b /= config.tmp_width;
                g /= config.tmp_width;
                r /= config.tmp_width;

                data[(config.width * y * 4 + i * 4 + 0) as usize] = b as u8;
                data[(config.width * y * 4 + i * 4 + 1) as usize] = g as u8;
                data[(config.width * y * 4 + i * 4 + 2) as usize] = r as u8;
            }
        }

        preview_src
            .push_buffer(outbuffer.copy_deep().unwrap())
            .into_result()
            .unwrap();
    }

    input_pipeline.set_state(gst::State::Null).into_result().unwrap();
    preview_pipeline.set_state(gst::State::Null).into_result().unwrap();
    output_pipeline.set_state(gst::State::Null).into_result().unwrap();
}
