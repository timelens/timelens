extern crate gstreamer as gst;
use gst::prelude::*;

fn main() {
    // Initialize GStreamer
    gst::init().unwrap();

    let uri = "file:///home/seb/library/movies/Brave/Brave.2012.1080p.BRrip.x264.YIFY.mp4";
    //let uri = "file:///home/seb/library/movies/Blender\\ Shorts/elephants-dream.avi";

    //let playbin = gst::ElementFactory::make("playbin", "playbin").unwrap();
    //playbin.set_property("uri", &uri);

    let pipeline = gst::parse_launch(&format!("playbin uri={}", uri)).unwrap();
    //let pipeline = gst::parse_launch(&format!("filesrc location={} ! decodebin", uri)).unwrap();
    //let pipeline = gst::parse_launch(&format!("filesrc location={}", uri)).unwrap();

    //let pipeline = gst::Pipeline::new(None);
    //let src = gst::ElementFactory::make("filesrc", None).unwrap();
    //let decodebin = gst::ElementFactory::make("decodebin", None).unwrap();

    //src.set_property("location", &uri);

    //pipeline.add_many(&[&src, &decodebin]);
    //gst::Element::link_many(&[&src, &decodebin]);

    pipeline.set_state(gst::State::Paused);
    pipeline.get_state(10*gst::SECOND);

    let duration: gst::ClockTime = pipeline.query_duration().unwrap();
    //let mseconds:  = duration.mseconds().unwrap();
    println!("{}", duration);

    //// Build the pipeline
    //let pipeline = gst::parse_launch(&format!("pipeline uri={}", uri)).unwrap();

    //// Start playing
    //let ret = pipeline.set_state(gst::State::Playing);
    //assert_ne!(ret, gst::StateChangeReturn::Failure);

    //pipeline.get_bus().unwrap().add_signal_watch();

    //let width = 1000.0;
    //let t = duration/width/1000;
    let mut i = 0;

    loop {
        //println!("{}", i*t);
        pipeline.seek_simple(gst::SeekFlags::FLUSH, i*10*gst::SECOND).unwrap();
        pipeline.get_state(10*gst::SECOND);
        i += 1;
    }

    //// Wait until error or EOS
    //let bus = pipeline.get_bus().unwrap();
    //while let Some(msg) = bus.timed_pop(gst::CLOCK_TIME_NONE) {
    //    use gst::MessageView;

    //    match msg.view() {
    //        MessageView::Eos(..) => break,
    //        MessageView::Error(err) => {
    //            println!(
    //                "Error from {:?}: {} ({:?})",
    //                "",//err.get_src().map(|s| s.get_path_string()),
    //                err.get_error(),
    //                err.get_debug()
    //                );
    //            break;
    //        }
    //        _ => (),
    //    }
    //}

    //// Shutdown pipeline
    //let ret = pipeline.set_state(gst::State::Null);
    //assert_ne!(ret, gst::StateChangeReturn::Failure);
}
