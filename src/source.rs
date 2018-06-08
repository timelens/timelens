extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;
use gst::prelude::*;

use std::fs;
use std::path::PathBuf;

pub struct VideoSource {
    pub height: usize,
    pub width: usize,
    pub duration: f32,
    pipeline: gst::Pipeline,
    appsink: gst_app::AppSink,
}

type Frame = gst::Sample;

impl VideoSource {
    pub fn new(filename: &String, output_height: usize, n: usize) -> VideoSource {
        let (width, height, duration) = get_meta(&filename);

        let aspect_ratio = 1000 * width / height;
        let output_width = output_height * aspect_ratio / 1000;

        let (pipeline, capsfilter, appsink) =
            build_pipeline(&filename, output_width, output_height);

        // set the input pipeline to paused to fill the buffers
        pipeline
            .set_state(gst::State::Paused)
            .into_result()
            .unwrap();
        pipeline.get_state(10 * gst::SECOND);

        let seek_mode = false;

        // if we don't seek, start playing
        if !seek_mode {
            pipeline
                .set_state(gst::State::Playing)
                .into_result()
                .unwrap();
        }

        let fps = gst::Fraction::new(n as i32, duration as i32); // FIXME

        capsfilter
            .set_property(
                "caps",
                &gst::Caps::new_simple(
                    "video/x-raw",
                    &[
                        ("format", &"BGRx"),
                        ("framerate", &fps),
                        ("width", &(output_width as i32)),
                        ("height", &(output_height as i32)),
                    ],
                ),
            )
            .unwrap();

        VideoSource {
            width: output_width,
            height: output_height,
            duration,
            pipeline: pipeline,
            appsink,
        }
    }
}

impl Iterator for VideoSource {
    type Item = Frame;

    fn next(&mut self) -> Option<Frame> {
        match self.appsink.pull_sample() {
            Some(sample) => Some(sample),
            None => {
                // we are probably at the end
                self.pipeline
                    .set_state(gst::State::Null)
                    .into_result()
                    .unwrap();

                None
            }
        }

        /*
        let progress = 100 * next_column / config.width;
        print!("\rtimelens: {}% ", progress);
        stdout().flush().unwrap();

        next_column += 1;

        if config.seek_mode {
            let j = (duration.nseconds().unwrap() as usize) / config.width * next_column;

            pipeline
                .seek_simple(
                    gst::SeekFlags::FLUSH, // | gst::SeekFlags::KEY_UNIT,
                    (j as u64) * gst::NSECOND,
                )
                .unwrap();
        }
        */
    }
}

// get resolution and duration of the input file
fn get_meta(filename: &String) -> (usize, usize, f32) {
    // generate file:// URI from an absolute filename
    let uri = format!(
        "file://{}",
        fs::canonicalize(&PathBuf::from(filename.as_str()))
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
        .unwrap() as usize;
    let height = caps.get_structure(0)
        .unwrap()
        .get_value("height")
        .unwrap()
        .get::<i32>()
        .unwrap() as usize;
    let duration_clocktime: gst::ClockTime = pipeline.query_duration().unwrap();
    let duration = duration_clocktime.nseconds().unwrap() as f32 / 1_000_000_000.0;

    pipeline.set_state(gst::State::Null).into_result().unwrap();

    (width, height, duration)
}

// build a pipeline that decodes the video to BGRx at 1 FPS, scales the frames to thumbnail size,
// and hands it to an Appsink
fn build_pipeline(
    filename: &String,
    output_width: usize,
    output_height: usize,
) -> (gst::Pipeline, gst::Element, gst_app::AppSink) {
    let uri = format!(
        "file://{}",
        fs::canonicalize(&PathBuf::from(filename.as_str()))
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
                    ("width", &(output_width as i32)),
                    ("height", &(output_height as i32)),
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
