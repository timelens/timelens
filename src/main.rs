extern crate clap;
use clap::{Arg, App};
extern crate glib;
extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;
use gst::prelude::*;
use std::cmp;

#[derive(Debug)]
struct Config {
    width: u64,
    height: u64,
    input_filename: String,
    output_filename: String,
}

fn parse_config() -> Config {
    let matches = App::new("nordlicht")
                           //.version("0.1")
                           .author("Sebastian Morr <sebastian@morr.cc>")
                           .arg(Arg::with_name("input")
                                .help("Input file")
                                .index(1))
                           .arg(Arg::with_name("width")
                                .help("Width of output")
                                .short("w")
                                .long("width")
                                .takes_value(true))
                           .arg(Arg::with_name("height")
                                .help("Height of output")
                                .short("h")
                                .long("height")
                                .takes_value(true))
                           .arg(Arg::with_name("output")
                                .help("Name of output file")
                                .short("o")
                                .long("output")
                                .takes_value(true))
                           .get_matches();

    let width_string = matches.value_of("width").unwrap_or("1920");
    let width: u64 = width_string.parse().expect("Invalid width");
    //    Ok(w) => w,
    //    Err(_) => {
    //        eprintln!("'{}' is not a valid width", width_string);
    //        return
    //    }
    //};

    let height_string = matches.value_of("height").unwrap_or("192");
    let height: u64 = height_string.parse().expect("Invalid height");
    //    Ok(w) => w,
    //    Err(_) => {
    //        eprintln!("'{}' is not a valid height", height_string);
    //        return
    //    }
    //};

    let input_filename = matches.value_of("input").unwrap_or("/home/seb/library/movies/Blender Shorts/big-buck-bunny.avi");

    let fallback_output = format!("{}.nordlicht.jpg", &input_filename);
    let output_filename = matches.value_of("output").unwrap_or(&fallback_output);

    Config {
        width: width,
        height: height,
        input_filename: String::from(input_filename),
        output_filename: String::from(output_filename),
    }
}

fn main() {
    let config = parse_config();
    println!("{:?}", config);

    let width2 = 100u64;

    //let file = env::args().nth(1).unwrap_or(String::from(
    //    "/home/seb/library/movies/Blender Shorts/big-buck-bunny.avi",
    //));

    let uri = format!("file://{}", config.input_filename);

    // Initialize GStreamer
    gst::init().unwrap();

    let src = gst::ElementFactory::make("uridecodebin", None).unwrap();
    src.set_property("uri", &uri).unwrap();

    let videorate = gst::ElementFactory::make("videorate", None).unwrap();
    let videoscale = gst::ElementFactory::make("videoscale", None).unwrap();
    videoscale.set_property("add-borders", &false).unwrap();
    let videoconvert = gst::ElementFactory::make("videoconvert", None).unwrap();

    let capsfilter = gst::ElementFactory::make("capsfilter", None).unwrap();
    capsfilter.set_property(
        "caps",
        &gst::Caps::new_simple(
            "video/x-raw",
            &[
                ("format", &"BGRx"),
                ("framerate", &gst::Fraction::new(1, 1)),
                ("width", &(width2 as i32)),
                ("height", &(config.height as i32)),
            ],
        ),
    ).unwrap();

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

    ///////////////////////////////////////////////////////////////////////
    let preview_pipeline = gst::Pipeline::new(None);

    let src2 = gst::ElementFactory::make("appsrc", None).unwrap();

    let capsfilter2 = gst::ElementFactory::make("capsfilter", None).unwrap();
    capsfilter2.set_property(
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
    ).unwrap();
    let videoconvert2 = gst::ElementFactory::make("videoconvert", None).unwrap();

    let sink2 = gst::ElementFactory::make("autovideosink", None).unwrap();
    sink2.set_property("sync", &false).unwrap();

    preview_pipeline
        .add_many(&[&src2, &capsfilter2, &videoconvert2, &sink2])
        .unwrap();
    gst::Element::link_many(&[&src2, &capsfilter2, &videoconvert2, &sink2]).unwrap();

    let appsrc = src2.clone()
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
    ///////////////////////////////////////////////////////////////////////
    let output_pipeline = gst::Pipeline::new(None);

    let src3 = gst::ElementFactory::make("appsrc", None).unwrap();

    let capsfilter3 = gst::ElementFactory::make("capsfilter", None).unwrap();
    capsfilter3.set_property(
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
    ).unwrap();

    let jpegenc = gst::ElementFactory::make("jpegenc", None).unwrap();
    let filesink = gst::ElementFactory::make("filesink", None).unwrap();
    filesink.set_property("location", &config.output_filename).unwrap();
    output_pipeline
        .add_many(&[&src3, &capsfilter3, &jpegenc, &filesink])
        .unwrap();
    gst::Element::link_many(&[&src3, &capsfilter3, &jpegenc, &filesink]).unwrap();

    let appsrc2 = src3.clone()
        .dynamic_cast::<gst_app::AppSrc>()
        .expect("Sink element is expected to be an appsrc!");
    appsrc2.set_property_format(gst::Format::Time);
    appsrc2.set_property_block(true);

    ///////////////////////////////////////////////////////////////////////

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

    pipeline.set_state(gst::State::Playing).into_result().unwrap();

    pipeline.get_state(10 * gst::SECOND);

    let duration: gst::ClockTime = pipeline.query_duration().unwrap();
    let fps = gst::Fraction::new(config.width as i32, duration.seconds().unwrap() as i32);
    println!("fps: {}", fps);

    capsfilter
        .set_property(
            "caps",
            &gst::Caps::new_simple(
                "video/x-raw",
                &[
                    ("format", &"BGRx"),
                    ("framerate", &fps),
                    ("width", &(width2 as i32)),
                    ("height", &(config.height as i32)),
                ],
            ),
        )
        .unwrap();
    //capsfilter2.set_property("caps", &gst::Caps::new_simple("video/x-raw", &[("format", &"BGRx"), ("framerate", &fps), ("width", &(width as i32)), ("height", &(height as i32))])).unwrap();
    //capsfilter3.set_property("caps", &gst::Caps::new_simple("video/x-raw", &[("format", &"BGRx"), ("framerate", &fps), ("width", &(width as i32)), ("height", &(height as i32))])).unwrap();

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

    let bus2 = preview_pipeline.get_bus().unwrap();
    bus2.connect_message(move |_, msg| match msg.view() {
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
    bus2.add_signal_watch();

    let bus3 = output_pipeline.get_bus().unwrap();
    bus3.connect_message(move |_, msg| match msg.view() {
        gst::MessageView::Eos(_) => println!("got eos"),
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
    bus3.add_signal_watch();

    let mut outbuffer = gst::Buffer::with_size((config.width * config.height * 4) as usize).unwrap();

    loop {
        let sample = match appsink.pull_sample() {
            None => {
                println!("got none");
                match output_pipeline.set_state(gst::State::Playing) {
                    gst::StateChangeReturn::Success => println!("success"),
                    gst::StateChangeReturn::Failure => println!("failure"),
                    gst::StateChangeReturn::Async => println!("async"),
                    gst::StateChangeReturn::NoPreroll => println!("nopreroll"),
                    _ => println!("other"),
                }
                match appsrc2.push_buffer(outbuffer.copy_deep().unwrap()) {
                    gst::FlowReturn::Ok => println!("ok"),
                    gst::FlowReturn::Flushing => println!("flushing"),
                    gst::FlowReturn::Eos => println!("eos"),
                    _ => println!("other"),
                }
                match appsrc2.end_of_stream() {
                    gst::FlowReturn::Ok => println!("ok"),
                    gst::FlowReturn::Flushing => println!("flushing"),
                    gst::FlowReturn::Eos => println!("eos"),
                    _ => println!("other"),
                }
                //let ret3 = output_pipeline.set_state(gst::State::Null);
                //assert_ne!(ret3, gst::StateChangeReturn::Failure);
                //output_pipeline.get_state(10 * gst::SECOND);
                break;
            }
            Some(sample) => sample,
        };

        let buffer = if let Some(buffer) = sample.get_buffer() {
            let pts = buffer.get_pts();
            println!("{}", pts);

            buffer
        } else {
            return;
        };

        let map = if let Some(map) = buffer.map_readable() {
            map
        } else {
            return;
        };

        let indata = map.as_slice();

        let pts: gst::ClockTime = buffer.get_pts();
        let i = cmp::min(
            (config.width * pts.nseconds().unwrap() / duration.nseconds().unwrap()),
            config.width - 1,
        );

        {
            let outbuffer = outbuffer.get_mut().unwrap();

            let mut data = outbuffer.map_writable().unwrap();

            for y in 0..config.height {
                let mut b: u64 = 0;
                let mut g: u64 = 0;
                let mut r: u64 = 0;

                for x in 0..width2 {
                    b += indata[(width2*y*4+4*x+0) as usize] as u64;
                    g += indata[(width2*y*4+4*x+1) as usize] as u64;
                    r += indata[(width2*y*4+4*x+2) as usize] as u64;
                }

                b /= width2;
                g /= width2;
                r /= width2;

                data[(config.width * y * 4 + i * 4 + 0) as usize] = b as u8;
                data[(config.width * y * 4 + i * 4 + 1) as usize] = g as u8;
                data[(config.width * y * 4 + i * 4 + 2) as usize] = r as u8;
            }
        }

        appsrc.push_buffer(outbuffer.copy_deep().unwrap()).into_result().unwrap();
    }

    let ret = pipeline.set_state(gst::State::Null);
    assert_ne!(ret, gst::StateChangeReturn::Failure);
    let ret2 = preview_pipeline.set_state(gst::State::Null);
    assert_ne!(ret2, gst::StateChangeReturn::Failure);
    let ret3 = output_pipeline.set_state(gst::State::Null);
    assert_ne!(ret3, gst::StateChangeReturn::Failure);
}
