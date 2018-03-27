extern crate gstreamer as gst;
use gst::prelude::*;

fn main() {
    // Initialize GStreamer
    gst::init().unwrap();

    let uri = "file:///home/seb/library/movies/Brave/Brave.2012.1080p.BRrip.x264.YIFY.mp4";
    //let uri = "/home/seb/library/movies/Brave/Brave.2012.1080p.BRrip.x264.YIFY.mp4";
    //let uri = "/home/seb/library/movies/Blender\\ Shorts/big-buck-bunny.avi";

    //let pipeline = gst::parse_launch(&format!("playbin uri={}", uri)).unwrap();
    //let pipeline = gst::parse_launch(&format!("filesrc location={} ! decodebin ! queue ! videoconvert ! videoscale ! video/x-raw,width=100,height=100 ! appsink

    let playbin = gst::ElementFactory::make("playbin", None).unwrap();

    let src = gst::ElementFactory::make("videotestsrc", None).unwrap();
    let src = gst::ElementFactory::make("filesrc", None).unwrap();
    src.set_property("location", &uri);
    let decodebin = gst::ElementFactory::make("decodebin", None).unwrap();
    let videoconvert = gst::ElementFactory::make("videoconvert", None).unwrap();
    let sink = gst::ElementFactory::make("autovideosink", None).unwrap();

    playbin.set_property("video-sink", &sink);
    playbin.set_property("uri", &uri);

    let pipeline = gst::Pipeline::new(None);

    //pipeline.add_many(&[&src, &decodebin, &videoconvert, &sink]).unwrap();
    //gst::Element::link_many(&[&src, &decodebin, &videoconvert, &sink]).unwrap();

    pipeline.add_many(&[&playbin]).unwrap();
    //gst::Element::link_many(&[&src, &decodebin, &videoconvert, &sink]).unwrap();

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
                        pipeline.seek_simple(gst::SeekFlags::FLUSH, i*20*gst::SECOND).unwrap();
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
