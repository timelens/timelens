extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;

use crate::frame::gst::prelude::*;
use std::fs::File;

// Holds a GStreamer Buffer, and knows its size and (optionally) its presentation timestamp in
// seconds
pub struct Frame {
    pub buffer: gst::Buffer,
    pub width: usize,
    pub height: usize,
    pub pts: Option<f32>,
}

impl Frame {
    // Initialize a new empty frame of size width*height
    pub fn new(width: usize, height: usize) -> Self {
        let buffer = gst::Buffer::with_size(width * height * 4).expect("Could not create buffer");
        Self {
            buffer,
            width,
            height,
            pts: None,
        }
    }

    // Scale frame to width*height. Only supports horizontal compression so far.
    pub fn scale(&self, width: usize, height: usize) -> Self {
        if width == 1 {
            // First, scale to 1 pixel width
            let mut frame = Self::new(1, self.height);

            {
                let buffer = frame
                    .buffer
                    .get_mut()
                    .expect("Could not get mutable buffer");
                let mut data = buffer
                    .map_writable()
                    .expect("Could not get writable map of buffer");

                let map = self
                    .buffer
                    .map_readable()
                    .expect("Could not get readable map of buffer");
                let indata = map.as_slice();

                for y in 0..self.height {
                    let mut b: usize = 0;
                    let mut g: usize = 0;
                    let mut r: usize = 0;

                    for x in 0..self.width {
                        b += indata[self.width * y * 4 + 4 * x] as usize;
                        g += indata[self.width * y * 4 + 4 * x + 1] as usize;
                        r += indata[self.width * y * 4 + 4 * x + 2] as usize;
                    }

                    b /= self.width;
                    g /= self.width;
                    r /= self.width;

                    data[y * 4] = b as u8;
                    data[y * 4 + 1] = g as u8;
                    data[y * 4 + 2] = r as u8;
                    data[y * 4 + 3] = 255;
                }
            }

            // Then, scale to target height
            let mut frame2 = Self::new(width, height);

            {
                let buffer = frame2
                    .buffer
                    .get_mut()
                    .expect("Could not get mutable buffer, part two");
                let mut data = buffer
                    .map_writable()
                    .expect("Could not get writable map of buffer, part two");

                let map = frame
                    .buffer
                    .map_readable()
                    .expect("Could not get readable map of buffer, part two");
                let indata = map.as_slice();

                let factor = frame.height as f32 / height as f32;

                for y in 0..height {
                    let mut b: usize = 0;
                    let mut g: usize = 0;
                    let mut r: usize = 0;

                    let from = (factor * y as f32) as usize;
                    let mut to = (factor * (y + 1) as f32) as usize;

                    if to == from {
                        to = from + 1;
                    }

                    for y2 in from..to {
                        b += indata[frame.width * y2 * 4] as usize;
                        g += indata[frame.width * y2 * 4 + 1] as usize;
                        r += indata[frame.width * y2 * 4 + 2] as usize;
                    }

                    b /= to - from;
                    g /= to - from;
                    r /= to - from;

                    data[y * 4] = b as u8;
                    data[y * 4 + 1] = g as u8;
                    data[y * 4 + 2] = r as u8;
                    data[y * 4 + 3] = 255;
                }
            }

            frame2
        } else {
            let src = gst::ElementFactory::make("appsrc", None)
                .expect("Could not create appsrc for scaling pipeline");

            let capsfilter = gst::ElementFactory::make("capsfilter", None)
                .expect("Could not create capsfilter for scaling pipeline");
            capsfilter
                .set_property(
                    "caps",
                    &gst::Caps::new_simple(
                        "video/x-raw",
                        &[
                            ("format", &"BGRx"),
                            ("framerate", &gst::Fraction::new(1, 1)),
                            ("width", &(self.width as i32)),
                            ("height", &(self.height as i32)),
                        ],
                    ),
                )
                .expect("Could not set capsfilter properties for scaling pipeline");

            let videoscale = gst::ElementFactory::make("videoscale", None)
                .expect("Could not create videoscale for scaling pipeline");
            // Scale frames exactly to the desired size, don't add borders
            videoscale
                .set_property("add-borders", &false)
                .expect("Could not set videoscale property for scaling pipeline");
            // Use Sinc scaling algorithm, which produces better results when downsampling
            videoscale.set_property_from_str("method", "sinc");

            let capsfilter2 = gst::ElementFactory::make("capsfilter", None)
                .expect("Could not create capsfilter 2 for scaling pipeline");
            capsfilter2
                .set_property(
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
                )
                .expect("Could not set capsfilter 2 properties for scaling pipeline");
            let sink = gst::ElementFactory::make("appsink", None)
                .expect("Could not create appsink for scaling pipeline");

            let pipeline = gst::Pipeline::new(None);
            pipeline
                .add_many(&[&src, &capsfilter, &videoscale, &capsfilter2, &sink])
                .expect("Could not create scaling pipeline");
            gst::Element::link_many(&[&src, &capsfilter, &videoscale, &capsfilter2, &sink])
                .expect("Could not link scaling pipeline");

            let appsrc = src
                .clone()
                .dynamic_cast::<gst_app::AppSrc>()
                .expect("Sink element is expected to be an appsrc!");
            appsrc.set_property_format(gst::Format::Time);
            appsrc.set_property_block(true);

            let appsink = sink
                .clone()
                .dynamic_cast::<gst_app::AppSink>()
                .expect("Sink element is expected to be an appsink!");
            // Go as fast as possible :)
            appsink
                .set_property("sync", &false)
                .expect("Could not set set appsink property for scaling pipeline");

            pipeline
                .set_state(gst::State::Playing)
                .into_result()
                .expect("Could not start scaling pipeline");

            appsrc
                .push_buffer(
                    self.buffer
                        .copy_deep()
                        .expect("Could not deep copy buffer in scaling pipeline"),
                )
                .into_result()
                .expect("Could not make push_buffer into result");

            appsrc
                .end_of_stream()
                .into_result()
                .expect("Could not make EOS into result");

            let sample = appsink
                .pull_sample()
                .expect("Could not pull sample in scaling pipeline");

            pipeline
                .set_state(gst::State::Null)
                .into_result()
                .expect("Could not stop scaling pipeline");

            Self {
                buffer: sample
                    .get_buffer()
                    .expect("Could not get buffer from sample in scaling pipeline"),
                width,
                height,
                pts: Some(
                    sample
                        .get_buffer()
                        .expect("Could not get buffer from sample in scaling pipeline, part two")
                        .get_pts()
                        .nseconds()
                        .expect("Could not convert PTS to nanoseconds") as f32
                        / 1_000_000_000.0,
                ),
            }
        }
    }

    // Copy the `other` frame into `self`, with the top left at dx/dy
    pub fn copy(&mut self, other: &Self, dx: usize, dy: usize) {
        let mut data = self
            .buffer
            .get_mut()
            .expect("Could not get mutable buffer for copying")
            .map_writable()
            .expect("Could not get writable map for copying");

        let map = other
            .buffer
            .map_readable()
            .expect("Could not get readable map for copying");
        let indata = map.as_slice();

        for x in 0..other.width {
            for y in 0..other.height {
                data[((y + dy) * self.width + (x + dx)) * 4 + 0] =
                    indata[(y * other.width + x) * 4];
                data[((y + dy) * self.width + (x + dx)) * 4 + 1] =
                    indata[(y * other.width + x) * 4 + 1];
                data[((y + dy) * self.width + (x + dx)) * 4 + 2] =
                    indata[(y * other.width + x) * 4 + 2];
                data[((y + dy) * self.width + (x + dx)) * 4 + 3] =
                    indata[(y * other.width + x) * 4 + 3];
            }
        }
    }

    // Write frame to `filename` as a JPEG using GStreamer
    pub fn write_to(&self, filename: &str, quality: i32) -> Result<bool, String> {
        {
            match File::create(&filename) {
                Ok(file) => file,
                Err(e) => {
                    return Err(format!("Could not create '{}': {})", &filename, e));
                }
            };
        }

        let src =
            gst::ElementFactory::make("appsrc", None).expect("Could not create appsrc for writing");

        let capsfilter = gst::ElementFactory::make("capsfilter", None)
            .expect("Could not create capsfilter for writing");
        capsfilter
            .set_property(
                "caps",
                &gst::Caps::new_simple(
                    "video/x-raw",
                    &[
                        ("format", &"BGRx"),
                        ("framerate", &gst::Fraction::new(1, 1)),
                        ("width", &(self.width as i32)),
                        ("height", &(self.height as i32)),
                    ],
                ),
            )
            .expect("Could not set properties on capsfilter for writing");

        let jpegenc = gst::ElementFactory::make("jpegenc", None).expect("Could not create jpegenc");
        jpegenc
            .set_property("quality", &quality)
            .expect("Could not create quality element");
        let filesink =
            gst::ElementFactory::make("filesink", None).expect("Could not create filesink");
        filesink
            .set_property("location", &filename)
            .expect("Could not set property on filesink");

        let pipeline = gst::Pipeline::new(None);
        pipeline
            .add_many(&[&src, &capsfilter, &jpegenc, &filesink])
            .expect("Could not create writing pipeline");
        gst::Element::link_many(&[&src, &capsfilter, &jpegenc, &filesink])
            .expect("Could not link writing pipeline");

        let appsrc = src
            .clone()
            .dynamic_cast::<gst_app::AppSrc>()
            .expect("Sink element is expected to be an appsrc!");
        appsrc.set_property_format(gst::Format::Time);
        appsrc.set_property_block(true);

        pipeline
            .set_state(gst::State::Playing)
            .into_result()
            .expect("Could not start writing pipeline");

        appsrc
            .push_buffer(
                self.buffer
                    .copy_deep()
                    .expect("Could not deep copy buffer for writing"),
            )
            .into_result()
            .expect("Could not get result from deep copy for writing");

        appsrc
            .end_of_stream()
            .into_result()
            .expect("Could not make EOS into result for writing");

        let bus = pipeline.get_bus().expect("Could not get bus for writing");

        loop {
            match bus.timed_pop(gst::CLOCK_TIME_NONE) {
                None => {}
                Some(msg) => match msg.view() {
                    gst::MessageView::Eos(_) => {
                        break;
                    }
                    _ => {}
                },
            }
        }

        pipeline
            .set_state(gst::State::Null)
            .into_result()
            .expect("Could not stop writing pipeline");

        Ok(true)
    }
}
