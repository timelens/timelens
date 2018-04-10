extern crate gstreamer as gst;
use gst::prelude::*;
extern crate gstreamer_app as gst_app;
extern crate gstreamer_video as gst_video;

extern crate glib;

use std::thread;

use std::error::Error as StdError;

extern crate failure;
use failure::Error;

#[macro_use]
extern crate failure_derive;

#[derive(Debug, Fail)]
#[fail(display = "Missing element {}", _0)]
struct MissingElement(&'static str);

#[derive(Debug, Fail)]
#[fail(display = "Received error from {}: {} (debug: {:?})", src, error, debug)]
struct ErrorMessage {
    src: String,
    error: String,
    debug: Option<String>,
    #[cause]
    cause: glib::Error,
}

const WIDTH: usize = 320;
const HEIGHT: usize = 240;

fn create_pipeline2(appsrc: gst_app::AppSrc) -> Result<gst::Pipeline, Error> {
    let pipeline = gst::Pipeline::new(None);
    let src =
        gst::ElementFactory::make("videotestsrc", None).ok_or(MissingElement("videotestsrc"))?;
    let sink = gst::ElementFactory::make("appsink", None).ok_or(MissingElement("appsink"))?;

    pipeline.add_many(&[&src, &sink])?;
    src.link(&sink)?;

    let appsink = sink.clone()
        .dynamic_cast::<gst_app::AppSink>()
        .expect("Sink element is expected to be an appsink!");

    let info = gst_video::VideoInfo::new(gst_video::VideoFormat::Bgrx, WIDTH as u32, HEIGHT as u32)
        .fps(gst::Fraction::new(5, 1))
        .build()
        .expect("Failed to create video info");

    appsink.set_caps(&info.to_caps().unwrap());

    appsink.set_callbacks(
        gst_app::AppSinkCallbacks::new()
            .new_sample(move |appsink| {
                println!("new sample");
                let sample = match appsink.pull_sample() {
                    None => return gst::FlowReturn::Eos,
                    Some(sample) => sample,
                };

                let buffer = if let Some(buffer) = sample.get_buffer() {
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

                println!("new buffer");
                appsrc.push_buffer(buffer.copy_deep().unwrap());

                //let samples = if let Ok(samples) = map.as_slice().as_slice_of::<i16>() {
                //    samples
                //} else {
                //    gst_element_error!(
                //        appsink,
                //        gst::ResourceError::Failed,
                //        ("Failed to interprete buffer as S16 PCM")
                //    );

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

    Ok(pipeline)
}

fn create_pipeline() -> Result<(gst::Pipeline, gst_app::AppSrc), Error> {
    gst::init()?;

    let pipeline = gst::Pipeline::new(None);
    let src = gst::ElementFactory::make("appsrc", None).ok_or(MissingElement("appsrc"))?;
    let videoconvert =
        gst::ElementFactory::make("videoconvert", None).ok_or(MissingElement("videoconvert"))?;
    let sink =
        gst::ElementFactory::make("autovideosink", None).ok_or(MissingElement("autovideosink"))?;

    pipeline.add_many(&[&src, &videoconvert, &sink])?;
    gst::Element::link_many(&[&src, &videoconvert, &sink])?;

    let appsrc = src.clone()
        .dynamic_cast::<gst_app::AppSrc>()
        .expect("Source element is expected to be an appsrc!");

    let info = gst_video::VideoInfo::new(gst_video::VideoFormat::Bgrx, WIDTH as u32, HEIGHT as u32)
        .fps(gst::Fraction::new(5, 1))
        .build()
        .expect("Failed to create video info");

    appsrc.set_caps(&info.to_caps().unwrap());
    appsrc.set_property_format(gst::Format::Time);
    appsrc.set_max_bytes(1);
    appsrc.set_property_block(true);

    Ok((pipeline, appsrc))
}

fn main_loop(pipeline: gst::Pipeline, pipeline2: gst::Pipeline) -> Result<(), Error> {
    //thread::spawn(move || {
    //    for i in 0..100 {
    //        println!("Producing frame {}", i);

    //        let r = if i % 2 == 0 { 0 } else { 255 };
    //        let g = if i % 3 == 0 { 0 } else { 255 };
    //        let b = if i % 5 == 0 { 0 } else { 255 };

    //        let mut buffer = gst::Buffer::with_size(WIDTH * HEIGHT * 4).unwrap();
    //        {
    //            let buffer = buffer.get_mut().unwrap();
    //            buffer.set_pts(i * 500 * gst::MSECOND);

    //            let mut data = buffer.map_writable().unwrap();

    //            for p in data.as_mut_slice().chunks_mut(4) {
    //                assert_eq!(p.len(), 4);
    //                p[0] = b;
    //                p[1] = g;
    //                p[2] = r;
    //                p[3] = 0;
    //            }
    //        }

    //        if appsrc.push_buffer(buffer) != gst::FlowReturn::Ok {
    //            break;
    //        }
    //    }

    //    let _ = appsrc.end_of_stream();
    //});

    pipeline.set_state(gst::State::Playing).into_result()?;
    pipeline2.set_state(gst::State::Playing).into_result()?;

    let bus = pipeline
        .get_bus()
        .expect("Pipeline without bus. Shouldn't happen!");

    while let Some(msg) = bus.timed_pop(gst::CLOCK_TIME_NONE) {
        use gst::MessageView;

        match msg.view() {
            MessageView::Eos(..) => break,
            MessageView::Error(err) => {
                pipeline.set_state(gst::State::Null).into_result()?;
                Err(ErrorMessage {
                    src: err.get_src()
                        .map(|s| s.get_path_string())
                        .unwrap_or_else(|| String::from("None")),
                    error: err.get_error().description().into(),
                    debug: err.get_debug(),
                    cause: err.get_error(),
                })?;
            }
            _ => (),
        }
    }

    pipeline.set_state(gst::State::Null).into_result()?;

    Ok(())
}

fn main() {
    let (pipeline, appsrc) = create_pipeline().unwrap();
    let pipeline2 = create_pipeline2(appsrc).unwrap();
    match main_loop(pipeline, pipeline2) {
        Ok(r) => r,
        Err(e) => eprintln!("Error! {}", e),
    }
}
