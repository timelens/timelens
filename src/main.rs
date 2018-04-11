extern crate glib;
extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;
use gst::prelude::*;
use std::env;
use std::cmp;

fn main() {
    let width = 597;
    let height = 400u64;

    let file = env::args().nth(1).unwrap_or(String::from(
        "/home/seb/library/movies/Blender Shorts/big-buck-bunny.avi",
    ));

    let uri = format!("file://{}", file);

    // Initialize GStreamer
    gst::init().unwrap();

    let src = gst::ElementFactory::make("uridecodebin", None).unwrap();
    src.set_property("uri", &uri).unwrap();

    let videorate = gst::ElementFactory::make("videorate", None).unwrap();
    let videoscale = gst::ElementFactory::make("videoscale", None).unwrap();
    let videoconvert = gst::ElementFactory::make("videoconvert", None).unwrap();

    let capsfilter = gst::ElementFactory::make("capsfilter", None).unwrap();
    capsfilter.set_property(
        "caps",
        &gst::Caps::new_simple(
            "video/x-raw",
            &[
                ("format", &"BGRx"),
                ("framerate", &gst::Fraction::new(1, 1)),
                ("width", &(1i32)),
                ("height", &(height as i32)),
            ],
        ),
    );

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
    appsink.set_property("sync", &false);

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
                ("width", &(width as i32)),
                ("height", &(height as i32)),
            ],
        ),
    );
    let videoconvert2 = gst::ElementFactory::make("videoconvert", None).unwrap();

    let sink2 = gst::ElementFactory::make("autovideosink", None).unwrap();
    sink2.set_property("sync", &false);

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
                ("width", &(width as i32)),
                ("height", &(height as i32)),
            ],
        ),
    );

    let jpegenc = gst::ElementFactory::make("jpegenc", None).unwrap();
    let filesink = gst::ElementFactory::make("filesink", None).unwrap();
    filesink.set_property("location", &"/tmp/nordlicht.jpg");
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

    pipeline.set_state(gst::State::Playing);

    pipeline.get_state(10 * gst::SECOND);

    let duration: gst::ClockTime = pipeline.query_duration().unwrap();
    let fps = gst::Fraction::new(width as i32, duration.seconds().unwrap() as i32);
    println!("fps: {}", fps);

    capsfilter
        .set_property(
            "caps",
            &gst::Caps::new_simple(
                "video/x-raw",
                &[
                    ("format", &"BGRx"),
                    ("framerate", &fps),
                    ("width", &(1i32)),
                    ("height", &(height as i32)),
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

    let mut outbuffer = gst::Buffer::with_size((width * height * 4) as usize).unwrap();

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
            println!("buffer received!");
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
            (width * pts.nseconds().unwrap() / duration.nseconds().unwrap()),
            width - 1,
        );

        {
            let outbuffer = outbuffer.get_mut().unwrap();

            let mut data = outbuffer.map_writable().unwrap();

            for y in 0..height {
                let mut dst = &mut data
                    [(width * y * 4 + i * 4) as usize..(width * y * 4 + i * 4 + 3) as usize];
                let src = &indata[(y * 4) as usize..(y * 4 + 3) as usize];
                dst.copy_from_slice(src);
            }
        }

        match appsrc.push_buffer(outbuffer.copy_deep().unwrap()) {
            gst::FlowReturn::Ok => println!("ok"),
            gst::FlowReturn::Flushing => println!("flushing"),
            gst::FlowReturn::Eos => println!("eos"),
            _ => println!("other"),
        }
    }

    let ret = pipeline.set_state(gst::State::Null);
    assert_ne!(ret, gst::StateChangeReturn::Failure);
    let ret2 = preview_pipeline.set_state(gst::State::Null);
    assert_ne!(ret2, gst::StateChangeReturn::Failure);
    let ret3 = output_pipeline.set_state(gst::State::Null);
    assert_ne!(ret3, gst::StateChangeReturn::Failure);
}
