extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;
use gst::prelude::*;

fn main() {
    // Initialize GStreamer
    gst::init().unwrap();

    let uri = "file:///home/seb/library/movies/Brave/Brave.2012.1080p.BRrip.x264.YIFY.mp4";

    let src = gst::ElementFactory::make("uridecodebin", None).unwrap();
    src.set_property("uri", &uri).unwrap();
    let videoconvert = gst::ElementFactory::make("videoconvert", None).unwrap();
    //let sink = gst::ElementFactory::make("autovideosink", None).unwrap();
    let sink = gst::ElementFactory::make("appsink", None).unwrap();

     let appsink = sink.clone()
        .dynamic_cast::<gst_app::AppSink>()
        .expect("Sink element is expected to be an appsink!");

    let pipeline = gst::Pipeline::new(None);

    pipeline.add_many(&[&src, &videoconvert, &sink]).unwrap();
    gst::Element::link_many(&[&videoconvert, &sink]).unwrap();

    // Connect the pad-added signal
    let pipeline_clone = pipeline.clone();
    let convert_clone = videoconvert.clone();
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

    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::new()
            .new_sample(|appsink| {
                let sample = match appsink.pull_sample() {
                    None => return gst::FlowReturn::Eos,
                    Some(sample) => sample,
                };

                let buffer = if let Some(buffer) = sample.get_buffer() {
                    println!("buffer received!");
                    buffer
                } else {
                    //gst_element_error!(
                    //    appsink,
                    //    gst::ResourceError::Failed,
                    //    ("Failed to get buffer from appsink")
                    //);

                    return gst::FlowReturn::Error;
                };

                let map = if let Some(map) = buffer.map_readable() {
                    map
                } else {
                    //gst_element_error!(
                    //    appsink,
                    //    gst::ResourceError::Failed,
                    //    ("Failed to map buffer readable")
                    //);

                    return gst::FlowReturn::Error;
                };

                //let samples = if let Ok(samples) = map.as_slice().as_slice_of::<i16>() {
                //    samples
                //} else {
                //    gst_element_error!(
                //        appsink,
                //        gst::ResourceError::Failed,
                //        ("Failed to interprete buffer as S16 PCM")
                //    );
                //
                //    return gst::FlowReturn::Error;
                //};

                //let sum: f64 = samples
                //    .iter()
                //    .map(|sample| {
                //        let f = f64::from(*sample) / f64::from(i16::MAX);
                //        f * f
                //    })
                //    .sum();
                //let rms = (sum / (samples.len() as f64)).sqrt();
                //println!("rms: {}", rms);

                gst::FlowReturn::Ok
            })
            .build(),
    );

    pipeline.set_state(gst::State::Playing);

    pipeline.get_state(10 * gst::SECOND);

    //
    //
    let mut i = 0;
    //
    ////let duration: gst::ClockTime = pipeline.query_duration().unwrap();
    //////let mseconds:  = duration.mseconds().unwrap();
    ////println!("{}", duration);
    //
    ////loop {
    ////    //println!("{}", i*t);
    ////    pipeline.seek_simple(gst::SeekFlags::FLUSH, i*10*gst::SECOND).unwrap();
    ////    pipeline.get_state(10*gst::SECOND);
    ////    i += 1;
    ////}
    //
    //// Wait until error or EOS
    let bus = pipeline.get_bus().unwrap();
    while let Some(msg) = bus.timed_pop(gst::CLOCK_TIME_NONE) {
        use gst::MessageView;
        //
        match msg.view() {
            //        MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                println!(
                    "Error from {:?}: {} ({:?})",
                    "", //err.get_src().map(|s| s.get_path_string()),
                    err.get_error(),
                    err.get_debug()
                );
                break;
            }
            MessageView::AsyncDone(..) => {
                let pos: gst::ClockTime = pipeline.query_position().unwrap();
                println!("async done: {}", pos);
                //let buffer = pipeline.emit("convert-sample", &[&gst::Caps::new_simple("image/png", &[("width", &(10i32))])]).unwrap();
                //let data = buffer.get_buffer();

                //pipeline.seek_simple(gst::SeekFlags::FLUSH | gst::SeekFlags::KEY_UNIT, i*gst::SECOND).unwrap();
                pipeline
                    .seek_simple(gst::SeekFlags::FLUSH, i * 20 * gst::SECOND)
                    .unwrap();
                //pipeline.seek_simple(gst::SeekFlags::FLUSH, i*gst::SECOND).unwrap();
                //pipeline.seek_simple(gst::SeekFlags::ACCURATE, i*5*gst::SECOND).unwrap();
                //pipeline.get_state(10*gst::SECOND);
                i += 1;
                println!("{}", i);
            }
            MessageView::DurationChanged(..) => {
                println!("duration");
                let dur: gst::ClockTime = pipeline.query_duration().unwrap();
                println!("{}", dur);
            }
            _ => {
                println!(".");
            }
        }
    }

    // Shutdown pipeline
    let ret = pipeline.set_state(gst::State::Null);
    assert_ne!(ret, gst::StateChangeReturn::Failure);
}
