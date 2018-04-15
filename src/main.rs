#[macro_use]
extern crate clap;
use clap::Arg;

extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;
use gst::prelude::*;

use std::cmp;

use std::io::Write;
use std::io::stdout;

use std::{thread, time};

#[derive(Debug)]
struct Config {
    width: usize,
    height: usize,
    thumb_width: usize,
    thumb_height: usize,
    thumb_columns: usize,
    input_filename: String,
    timeline_filename: String,
    thumbnails_filename: String,
    tmp_width: usize,
    preview: bool,
    seek_mode: bool,
}

fn parse_config() -> Config {
    let matches = app_from_crate!()
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
            Arg::with_name("timeline")
                .help("Name of timeline output file")
                .long("timeline")
                .takes_value(true),
        )
        .arg(
            Arg::with_name("thumbnails")
            .help("Name of thumbnails output file")
            .long("thumbnails")
            .takes_value(true),
            )
        .arg(
            Arg::with_name("preview")
                .help("Open a preview window")
                .short("p")
                .long("preview"),
        )
        .arg(Arg::with_name("seek").help("Allow seeking").long("seek"))
        .get_matches();

    let width_string = matches.value_of("width").unwrap_or("1000");
    let width: usize = width_string.parse().expect("Invalid width");

    let height_string = matches.value_of("height").unwrap_or("100");
    let height: usize = height_string.parse().expect("Invalid height");

    let input_filename = matches
        .value_of("input")
        .unwrap_or("/home/seb/library/movies/Blender Shorts/big-buck-bunny.avi");

    let fallback_output = format!("{}.timeline.jpg", &input_filename);
    let timeline_filename = matches.value_of("timeline").unwrap_or(&fallback_output);
    let fallback_output2 = format!("{}.thumbnails.jpg", &input_filename);
    let thumbnails_filename = matches.value_of("thumbnails").unwrap_or(&fallback_output2);

    Config {
        width,
        height,
        thumb_width: 160,
        thumb_height: height,
        thumb_columns: 20,
        input_filename: String::from(input_filename),
        timeline_filename: String::from(timeline_filename),
        thumbnails_filename: String::from(thumbnails_filename),
        tmp_width: 160,
        preview: matches.is_present("preview"),
        seek_mode: matches.is_present("seek"),
    }
}

fn build_input_pipeline(config: &Config) -> (gst::Pipeline, gst::Element, gst_app::AppSink) {
    let uri = format!("file://{}", config.input_filename);

    let src = gst::ElementFactory::make("uridecodebin", None).unwrap();
    src.set_property("uri", &uri).unwrap();

    let videoconvert = gst::ElementFactory::make("videoconvert", None).unwrap();
    let videorate = gst::ElementFactory::make("videorate", None).unwrap();
//    let videoconvert2 = gst::ElementFactory::make("videoconvert", None).unwrap();
//    let glupload = gst::ElementFactory::make("glupload", None).unwrap();
//    let glshader = gst::ElementFactory::make("glshader", None).unwrap();
//    glshader.set_property("fragment", &"
//#version 130
//
//#ifdef GL_ES
//precision mediump float;
//#endif
//
//varying vec2 v_texcoord;
//uniform sampler2D tex;
//
//void main () {
//    vec2 texturecoord = v_texcoord.xy;
//    vec4 avg = vec4(0.0);
//
//    ivec2 size = textureSize(tex, 0);
//    float in_width = float(size.x);
//
//    for(float x=0.0; x < in_width; x++) {
//        avg += texture2D(tex, vec2(x/in_width, v_texcoord.y));
//    }
//
//    avg /= in_width;
//
//    gl_FragColor = avg;
//}
//    ").unwrap();
//    glshader.set_property("vertex", &"
//#version 130
//
//attribute vec4 a_position;
//attribute vec2 a_texcoord;
//varying vec2 v_texcoord;
//
//void main() {
//    gl_Position = a_position;
//    v_texcoord = a_texcoord;
//}
//                          ").unwrap();
//    let gldownload = gst::ElementFactory::make("gldownload", None).unwrap();
    let videoscale = gst::ElementFactory::make("videoscale", None).unwrap();
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
                    ("width", &(config.thumb_width as i32)),
                    ("height", &(config.thumb_height as i32)),
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
    appsink.set_property("sync", &false).unwrap();

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
        .set_property("location", &config.timeline_filename)
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

fn build_output_pipeline2(config: &Config) -> (gst::Pipeline, gst_app::AppSrc) {
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
                ("width", &((config.thumb_width*config.thumb_columns) as i32)),
                ("height", &((config.thumb_height*(config.width/config.thumb_columns+1)) as i32)),
                ],
                ),
                )
        .unwrap();

    let jpegenc = gst::ElementFactory::make("jpegenc", None).unwrap();
    let filesink = gst::ElementFactory::make("filesink", None).unwrap();
    filesink
        .set_property("location", &config.thumbnails_filename)
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

    preview_pipeline
        .set_state(gst::State::Playing)
        .into_result()
        .unwrap();

    (preview_pipeline, appsrc)
}

fn build_preview_pipeline2(config: &Config) -> (gst::Pipeline, gst_app::AppSrc) {
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
                ("width", &((config.thumb_width*config.thumb_columns) as i32)),
                ("height", &((config.thumb_height*(config.width/config.thumb_columns+1)) as i32)),
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

    preview_pipeline
        .set_state(gst::State::Playing)
        .into_result()
        .unwrap();

    (preview_pipeline, appsrc)
}

fn generate_timeline_and_thumbnails(
    config: &Config,
    input_pipeline: &gst::Pipeline,
    appsink: &gst_app::AppSink,
    preview_src: &gst_app::AppSrc,
    preview_src2: &gst_app::AppSrc,
    duration: &gst::ClockTime,
) -> (gst::Buffer, gst::Buffer) {
    let mut timeline = gst::Buffer::with_size(config.width * config.height * 4).unwrap();

    let thumb_rows = config.width/config.thumb_columns + 1;
    let mut thumbnails = gst::Buffer::with_size(config.thumb_width*config.thumb_columns * config.thumb_height*thumb_rows * 4).unwrap();

    let mut done = vec![0; config.width];

    let mut next_column = 0;

    loop {
        let sample = match appsink.pull_sample() {
            None => {
                // we are probably at the end
                println!("eos?");
                return (timeline, thumbnails);
            }
            Some(sample) => sample,
        };

        let buffer = sample.get_buffer().unwrap();
        let map = buffer.map_readable().unwrap();
        let indata = map.as_slice();

        let pts: gst::ClockTime = buffer.get_pts();
        let i = cmp::min(
            config.width * (pts.nseconds().unwrap() as usize)
                / (duration.nseconds().unwrap() as usize),
            config.width - 1,
        );

        let progress = 100 * pts.nseconds().unwrap() / duration.nseconds().unwrap();
        print!("\rnordlicht: {}% ", progress);
        stdout().flush().unwrap();

        {
            let timeline = timeline.get_mut().unwrap();
            let mut data = timeline.map_writable().unwrap();

            for y in 0..config.height {
                let mut b: usize = 0;
                let mut g: usize = 0;
                let mut r: usize = 0;

                for x in 0..config.tmp_width {
                    b += indata[config.tmp_width * y * 4 + 4 * x] as usize;
                    g += indata[config.tmp_width * y * 4 + 4 * x + 1] as usize;
                    r += indata[config.tmp_width * y * 4 + 4 * x + 2] as usize;
                }

                b /= config.tmp_width;
                g /= config.tmp_width;
                r /= config.tmp_width;

                data[config.width * y * 4 + i * 4] = b as u8;
                data[config.width * y * 4 + i * 4 + 1] = g as u8;
                data[config.width * y * 4 + i * 4 + 2] = r as u8;
                data[config.width * y * 4 + i * 4 + 3] = 255;
            }
        }

        {
            let thumbnails = thumbnails.get_mut().unwrap();
            let mut data = thumbnails.map_writable().unwrap();

            let tx = i % config.thumb_columns;
            let ty = i / config.thumb_columns;

            for x in 0..config.thumb_width {
                for y in 0..config.thumb_height {
                    let r = indata[y*config.thumb_width*4+4*x] as usize;
                    let g = indata[y*config.thumb_width*4+4*x+1] as usize;
                    let b = indata[y*config.thumb_width*4+4*x+2] as usize;

                    data[(config.thumb_columns*config.thumb_width*4)*(ty*config.thumb_height+y) + (tx*config.thumb_width+x)*4] = r as u8;
                    data[(config.thumb_columns*config.thumb_width*4)*(ty*config.thumb_height+y) + (tx*config.thumb_width+x)*4+1] = g as u8;
                    data[(config.thumb_columns*config.thumb_width*4)*(ty*config.thumb_height+y) + (tx*config.thumb_width+x)*4+2] = b as u8;
                    data[(config.thumb_columns*config.thumb_width*4)*(ty*config.thumb_height+y) + (tx*config.thumb_width+x)*4+3] = 255 as u8;
                }
            }
        }

        if config.preview {
            preview_src
                .push_buffer(timeline.copy_deep().unwrap())
                .into_result()
                .unwrap();
            preview_src2
                .push_buffer(thumbnails.copy_deep().unwrap())
                .into_result()
                .unwrap();
        }

        done[i as usize] += 1;

        //for n in &done {
        //    print!("{}", n);
        //    stdout().flush().unwrap();
        //}

        if !done.contains(&0) {
            // we are done
            println!("done!");
            return (timeline, thumbnails);
        }

        if config.seek_mode {
            next_column += 1;

            let j = (duration.nseconds().unwrap() as usize) / config.width * next_column;

            input_pipeline
                .seek_simple(
                    gst::SeekFlags::FLUSH, // | gst::SeekFlags::KEY_UNIT,
                    (j as u64) * gst::NSECOND,
                )
                .unwrap();
        }
    }
}

fn write_result(
    timeline: &gst::Buffer,
    output_pipeline: &gst::Pipeline,
    output_src: &gst_app::AppSrc
) {
    output_pipeline
        .set_state(gst::State::Playing)
        .into_result()
        .unwrap();
    output_src
        .push_buffer(timeline.copy_deep().unwrap())
        .into_result()
        .unwrap();
    output_src.end_of_stream().into_result().unwrap();
}

fn main() {
    let config = parse_config();
    println!("{:#?}", config);

    // Initialize GStreamer
    gst::init().unwrap();

    let (input_pipeline, capsfilter, appsink) = build_input_pipeline(&config);
    let (output_pipeline, output_src) = build_output_pipeline(&config);
    let (output_pipeline2, output_src2) = build_output_pipeline2(&config);
    let (preview_pipeline, preview_src) = build_preview_pipeline(&config);
    let (preview_pipeline2, preview_src2) = build_preview_pipeline2(&config);

    input_pipeline
        .set_state(gst::State::Playing)
        .into_result()
        .unwrap();

    input_pipeline.get_state(10 * gst::SECOND);

    if !config.seek_mode {
        input_pipeline
            .set_state(gst::State::Playing)
            .into_result()
            .unwrap();
    }

    let duration: gst::ClockTime = input_pipeline.query_duration().unwrap();
    let fps = gst::Fraction::new(config.width as i32, duration.seconds().unwrap() as i32);

    capsfilter
        .set_property(
            "caps",
            &gst::Caps::new_simple(
                "video/x-raw",
                &[
                    ("format", &"BGRx"),
                    ("framerate", &fps),
                    ("width", &(config.thumb_width as i32)),
                    ("height", &(config.thumb_height as i32)),
                ],
            ),
        )
        .unwrap();

    for pipeline in &[&input_pipeline, &output_pipeline, &output_pipeline2, &preview_pipeline, &preview_pipeline2] {
        let bus = pipeline.get_bus().unwrap();
        bus.connect_message(move |_, msg| {
                            match msg.view() {
                                gst::MessageView::Eos(_) => {
                                    println!("eos received");
                                }
                                gst::MessageView::Error(err) => {
                                    eprintln!(
                                        "Error received from element {:?}: {}",
                                        err.get_src().map(|s| s.get_path_string()),
                                        err.get_error()
                                        );
                                    eprintln!("Debugging information: {:?}", err.get_debug());
                                }
                                _ => {
                                    //println!(".");
                                }
                            }
        }
        );
        bus.add_signal_watch();
    }

    let (timeline, thumbnails) = generate_timeline_and_thumbnails(&config, &input_pipeline, &appsink, &preview_src, &preview_src2, &duration);

    write_result(&timeline, &output_pipeline, &output_src);
    println!("-> '{}'", config.timeline_filename);

    write_result(&thumbnails, &output_pipeline2, &output_src2);
    println!("-> '{}'", config.thumbnails_filename);


    input_pipeline
        .set_state(gst::State::Null)
        .into_result()
        .unwrap();
    preview_pipeline
        .set_state(gst::State::Null)
        .into_result()
        .unwrap();
    preview_pipeline2
        .set_state(gst::State::Null)
        .into_result()
        .unwrap();
    output_pipeline
        .set_state(gst::State::Null)
        .into_result()
        .unwrap();
    output_pipeline2
        .set_state(gst::State::Null)
        .into_result()
        .unwrap();

    output_pipeline2.get_state(10 * gst::SECOND);

    let sec = time::Duration::from_secs(5);
    thread::sleep(sec);
}
