extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;

use frame;
use source::gst::prelude::*;
use std::fs;
use std::path::PathBuf;

// A reference to exactly one of a file's video streams.
//
// This type provides an `Iterator` interface which returns frames from the video in an
// unspecified order.
pub struct VideoSource {
    // Height of the output frames
    pub height: usize,
    // Width of the output frames
    pub width: usize,
    // Duration of the video in seconds
    pub duration: f32,

    pipeline: gst::Pipeline,
    appsink: gst_app::AppSink,
    seek_mode: bool,
    n: usize,
    next_column: usize,
}

impl VideoSource {
    // Initializes a new `VideoSource`, referencing the specified video `filename`.
    //
    // Any frames this source outputs will be `output_height` pixels high. The source will try to
    // output approximately `n` frames.
    pub fn new(filename: &str, output_height: usize, n: usize) -> VideoSource {
        // Initialize GStreamer
        gst::init().unwrap();

        // Get size and duration information
        let (aspect_ratio, duration) = get_meta(&filename);

        // Calculate which output width keeps the aspect ratio
        let output_width = (output_height as f32 * aspect_ratio) as usize;

        // Set up GStreamer pipeline
        let (pipeline, capsfilter, appsink) =
            build_pipeline(&filename, output_width, output_height);

        // Set the input pipeline to paused to fill the buffers
        pipeline
            .set_state(gst::State::Paused)
            .into_result()
            .unwrap();
        pipeline.get_state(10 * gst::SECOND);

        let seek_mode = false;

        // If we don't seek, start playing
        if !seek_mode {
            pipeline
                .set_state(gst::State::Playing)
                .into_result()
                .unwrap();
        }

        // Approximate which FPS value is required to output n frames in total
        let fps = gst::Fraction::new(n as i32, duration as i32);

        // Set the capsfilter element correctly so that the pipeline will output the correct format
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

        // Return the new VideoSource
        VideoSource {
            width: output_width,
            height: output_height,
            duration,
            pipeline,
            seek_mode: false,
            appsink,
            n,
            next_column: 0,
        }
    }
}

impl Iterator for VideoSource {
    type Item = frame::Frame;

    fn next(&mut self) -> Option<frame::Frame> {
        if self.seek_mode {
            let j = (self.duration * 1_000_000_000.0) / self.n as f32 * self.next_column as f32;

            self.pipeline
                .seek_simple(
                    gst::SeekFlags::FLUSH, // | gst::SeekFlags::KEY_UNIT,
                    (j as u64) * gst::NSECOND,
                )
                .unwrap();
        }

        match self.appsink.pull_sample() {
            Some(sample) => {
                self.next_column += 1;
                Some(frame::Frame {
                    buffer: sample.get_buffer().unwrap(),
                    width: self.width,
                    height: self.height,
                    pts: Some(
                        sample.get_buffer().unwrap().get_pts().nseconds().unwrap() as f32
                            / 1_000_000_000.0,
                    ),
                })
            }
            None => {
                // We are at the end of the video. Stop pipeline and return None.
                self.pipeline
                    .set_state(gst::State::Null)
                    .into_result()
                    .unwrap();

                None
            }
        }
    }
}

// Get resolution and duration of the input file
fn get_meta(filename: &str) -> (f32, f32) {
    // Generate file:// URI from an absolute filename
    let uri = format!(
        "file://{}",
        fs::canonicalize(&PathBuf::from(filename))
            .unwrap()
            .to_str()
            .unwrap()
    );

    // Set up a playbin element, which automatically select decoders
    let playbin = gst::ElementFactory::make("playbin", None).unwrap();
    playbin.set_property("uri", &uri).unwrap();

    // We don't actually want any output, so we connect the playbin to fakesinks
    let fakesink = gst::ElementFactory::make("fakesink", None).unwrap();
    let fakesink2 = gst::ElementFactory::make("fakesink", None).unwrap();
    playbin.set_property("video-sink", &fakesink).unwrap();
    playbin.set_property("audio-sink", &fakesink2).unwrap();

    // Create a pipeline and add the playbin to it
    let pipeline = gst::Pipeline::new(None);
    pipeline.add(&playbin).unwrap();

    // Set pipeline state to "paused" to start pad negotiation
    pipeline
        .set_state(gst::State::Paused)
        .into_result()
        .unwrap();
    pipeline.get_state(10 * gst::SECOND);

    // Get the sinkpad of the first video stream
    let pad = playbin
        .emit("get-video-pad", &[&0])
        .unwrap()
        .unwrap()
        .get::<gst::Pad>()
        .unwrap();

    // And retrieve width and height from its caps
    let caps = pad.get_current_caps().unwrap();
    let width = caps
        .get_structure(0)
        .unwrap()
        .get_value("width")
        .unwrap()
        .get::<i32>()
        .unwrap() as usize;
    let height = caps
        .get_structure(0)
        .unwrap()
        .get_value("height")
        .unwrap()
        .get::<i32>()
        .unwrap() as usize;

    // Pixels aren't necessarily square, so we need to get their aspect ratio to calculate the
    // aspect ratio of the video
    let pixel_aspect_ratio = caps
        .get_structure(0)
        .unwrap()
        .get_value("pixel-aspect-ratio")
        .unwrap()
        .get::<gst::Fraction>()
        .unwrap();
    let aspect_ratio = width as f32 * *pixel_aspect_ratio.numer() as f32
        / height as f32
        / *pixel_aspect_ratio.denom() as f32;

    // Also, query the pipeline for the duration and convert to seconds
    let duration_clocktime: gst::ClockTime = pipeline.query_duration().unwrap();
    let duration = duration_clocktime.nseconds().unwrap() as f32 / 1_000_000_000.0;

    // Stop the pipeline again
    pipeline.set_state(gst::State::Null).into_result().unwrap();

    (aspect_ratio, duration)
}

// Build a pipeline that decodes the video to BGRx at 1 FPS, scales the frames to thumbnail size,
// and hands it to an Appsink
fn build_pipeline(
    filename: &str,
    output_width: usize,
    output_height: usize,
) -> (gst::Pipeline, gst::Element, gst_app::AppSink) {
    let uri = format!(
        "file://{}",
        fs::canonicalize(&PathBuf::from(filename))
            .unwrap()
            .to_str()
            .unwrap()
    );

    let src = gst::ElementFactory::make("uridecodebin", None).unwrap();
    src.set_property("uri", &uri).unwrap();

    let videoconvert = gst::ElementFactory::make("videoconvert", None).unwrap();
    let videorate = gst::ElementFactory::make("videorate", None).unwrap();
    let videoscale = gst::ElementFactory::make("videoscale", None).unwrap();
    // Scale frames exactly to the desired size, don't add borders
    videoscale.set_property("add-borders", &false).unwrap();
    // Use Sinc scaling algorithm, which produces better results when downsampling
    videoscale.set_property_from_str("method", "sinc");

    let method = String::from("sinc");

    let capsfilter = gst::ElementFactory::make("capsfilter", method.as_str()).unwrap();
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

    let appsink = sink
        .clone()
        .dynamic_cast::<gst_app::AppSink>()
        .expect("Sink element is expected to be an appsink!");
    // Go as fast as possible :)
    appsink.set_property("sync", &false).unwrap();

    // When a new source pad opens on the decodebin, connect it to the videoconvert element.
    // this code is required because media files might contain no (or many) video strems, this is
    // not known before the pipeline is started.
    let convert_clone = videoconvert.clone();
    src.connect_pad_added(move |_, src_pad| {
        let convert = &convert_clone;

        let sink_pad = convert
            .get_static_pad("sink")
            .expect("Failed to get static sink pad from convert");

        if sink_pad.is_linked() {
            // We are already linked. Ignoring.
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
