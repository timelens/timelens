extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;

use crate::frame;
use crate::source::gst::prelude::*;
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
    pub fn new(filename: &str, output_height: usize, n: usize) -> Result<Self, String> {
        // Initialize GStreamer
        gst::init().expect("Could not initialize GStreamer");

        // Get size and duration information
        let (aspect_ratio, duration) = get_meta(&filename)?;

        // Calculate which output width keeps the aspect ratio
        let output_width = (output_height as f32 * aspect_ratio) as usize;

        // Set up GStreamer pipeline
        let (pipeline, capsfilter, appsink) =
            build_pipeline(&filename, output_width, output_height);

        // Set the input pipeline to paused to fill the buffers
        pipeline
            .set_state(gst::State::Paused)
            .into_result()
            .expect("Could not pause input pipeline");
        pipeline.get_state(10 * gst::SECOND);

        let seek_mode = false;

        // If we don't seek, start playing
        if !seek_mode {
            pipeline
                .set_state(gst::State::Playing)
                .into_result()
                .expect("Could not start input pipeline");
        }

        // Approximate which FPS value is required to output n frames in total
        let fps = gst::Fraction::new((n * 100) as i32, (duration * 100.0) as i32);

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
            .expect("Could not set properties on input capsfilter");

        // Return the new VideoSource
        Ok(Self {
            width: output_width,
            height: output_height,
            duration,
            pipeline,
            seek_mode: false,
            appsink,
            n,
            next_column: 0,
        })
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
                .expect("Could not seek");
        }

        match self.appsink.pull_sample() {
            Some(sample) => {
                self.next_column += 1;
                Some(frame::Frame {
                    buffer: sample
                        .get_buffer()
                        .expect("Could not get buffer from input pipeline"),
                    width: self.width,
                    height: self.height,
                    pts: Some(
                        sample
                            .get_buffer()
                            .expect("Could not get buffer from input pipeline, again")
                            .get_pts()
                            .nseconds()
                            .expect("Could not convert PTS to nanoseconds in input pipeline")
                            as f32
                            / 1_000_000_000.0,
                    ),
                })
            }
            None => {
                // We are at the end of the video. Stop pipeline and return None.
                self.pipeline
                    .set_state(gst::State::Null)
                    .into_result()
                    .expect("Could not stop input pipeline");

                None
            }
        }
    }
}

// Get resolution and duration of the input file
fn get_meta(filename: &str) -> Result<(f32, f32), String> {
    // Generate file:// URI from an absolute filename
    let path = PathBuf::from(filename);

    if path.is_dir() {
        return Err(format!(
            "Input argument '{}' is a directory. Please specify a file.",
            &filename
        ));
    }

    if !path.is_file() {
        return Err(format!("Input file '{}' could not be found.", &filename));
    }

    let absolute = match fs::canonicalize(&path) {
        Ok(path) => path,
        Err(err) => {
            return Err(format!("Input file could not be opened: {}", &err));
        }
    };
    let absolute = absolute
        .to_str()
        .expect("Could not convert absolut path to str");
    let uri = format!("file://{}", absolute);

    // Set up a playbin element, which automatically select decoders
    let playbin = gst::ElementFactory::make("playbin", None).expect("Could not create playbin");
    playbin
        .set_property("uri", &uri)
        .expect("Could not set property on playbin");

    // We don't actually want any output, so we connect the playbin to fakesinks
    let fakesink = gst::ElementFactory::make("fakesink", None).expect("Could not create fakesink");
    let fakesink2 =
        gst::ElementFactory::make("fakesink", None).expect("Could not create fakesink 2");
    playbin
        .set_property("video-sink", &fakesink)
        .expect("Could not set property on fakesink");
    playbin
        .set_property("audio-sink", &fakesink2)
        .expect("Could not set property on fakesink 2");

    // Create a pipeline and add the playbin to it
    let pipeline = gst::Pipeline::new(None);
    pipeline
        .add(&playbin)
        .expect("Could not add playbin to pipeline");

    // Set pipeline state to "paused" to start pad negotiation
    match pipeline.set_state(gst::State::Paused).into_result() {
        Ok(_) => (),
        Err(_) => {
            return Err(String::from("Input file could not be opened"));
        }
    }
    pipeline.get_state(10 * gst::SECOND);

    // Get the sinkpad of the first video stream
    let pad = playbin
        .emit("get-video-pad", &[&0])
        .expect("Could not get video pad")
        .expect("Could not get video pad, part 2");

    let pad = if let Some(pad) = pad.get::<gst::Pad>() {
        pad
    } else {
        return Err(String::from("This does not seem to be a video file."));
    };

    // And retrieve width and height from its caps
    let caps = pad.get_current_caps().expect("Could not get current caps");
    let width = caps
        .get_structure(0)
        .expect("Could not get structure from caps width")
        .get_value("width")
        .expect("Could not get width from caps")
        .get::<i32>()
        .expect("Could not convert width to i32") as usize;
    let height = caps
        .get_structure(0)
        .expect("Could not get structure from caps height")
        .get_value("height")
        .expect("Could not get height from caps")
        .get::<i32>()
        .expect("Could not convert height to i32") as usize;

    // Pixels aren't necessarily square, so we need to get their aspect ratio to calculate the
    // aspect ratio of the video
    let pixel_aspect_ratio = caps
        .get_structure(0)
        .expect("Could not get structure from caps aspect ratio")
        .get_value("pixel-aspect-ratio")
        .expect("Could not get aspect ratio from caps")
        .get::<gst::Fraction>()
        .expect("Could not convert aspect ratio to fraction");
    let aspect_ratio = width as f32 * *pixel_aspect_ratio.numer() as f32
        / height as f32
        / *pixel_aspect_ratio.denom() as f32;

    // Also, query the pipeline for the duration and convert to seconds
    let duration_clocktime: gst::ClockTime =
        pipeline.query_duration().expect("Could not query duration");
    let duration = duration_clocktime
        .nseconds()
        .expect("Could not convert duration to nanoseconds") as f32
        / 1_000_000_000.0;

    // Stop the pipeline again
    pipeline
        .set_state(gst::State::Null)
        .into_result()
        .expect("Could not stop querying pipeline");

    Ok((aspect_ratio, duration))
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
            .expect("Could not canonicalize input filename")
            .to_str()
            .expect("Could not convert canonicalized input filename to str")
    );

    let src =
        gst::ElementFactory::make("uridecodebin", None).expect("Could not create uridecodebin");
    src.set_property("uri", &uri)
        .expect("Could not set property on uridecodebin");

    let videoconvert =
        gst::ElementFactory::make("videoconvert", None).expect("Could not create videoconvert");
    let videorate =
        gst::ElementFactory::make("videorate", None).expect("Could not convert videorate");
    let videoscale =
        gst::ElementFactory::make("videoscale", None).expect("Could not convert videoscale");
    // Scale frames exactly to the desired size, don't add borders
    videoscale
        .set_property("add-borders", &false)
        .expect("Could not set videoscape property");
    // Use Sinc scaling algorithm, which produces better results when downsampling
    videoscale.set_property_from_str("method", "sinc");

    let method = String::from("sinc");

    let capsfilter = gst::ElementFactory::make("capsfilter", method.as_str())
        .expect("Could not create input capsfilter");
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
        .expect("Could not set properties on input capsfilter");

    let sink = gst::ElementFactory::make("appsink", None).expect("Could not create input appsink");

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
        .expect("Could not add elements to input pipeline");

    gst::Element::link_many(&[&videoconvert, &videorate, &videoscale, &capsfilter, &sink])
        .expect("Could not link input pipeline");

    let appsink = sink
        .clone()
        .dynamic_cast::<gst_app::AppSink>()
        .expect("Sink element is expected to be an appsink!");
    // Go as fast as possible :)
    appsink
        .set_property("sync", &false)
        .expect("Could not set property on input appsink");

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
