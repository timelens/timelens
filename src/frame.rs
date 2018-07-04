extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;

use frame::gst::prelude::*;

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
    pub fn new(width: usize, height: usize) -> Frame {
        let buffer = gst::Buffer::with_size(width * height * 4).unwrap();
        Frame {
            buffer,
            width,
            height,
            pts: None,
        }
    }

    // Scale frame to width*height. Only supports horizontal compression so far.
    pub fn scale(&self, width: usize, height: usize) -> Frame {
        assert_eq!(1, width);

        // First, scale to 1 pixel width
        let mut frame = Frame::new(1, self.height);

        {
            let buffer = frame.buffer.get_mut().unwrap();
            let mut data = buffer.map_writable().unwrap();

            let map = self.buffer.map_readable().unwrap();
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
        let mut frame2 = Frame::new(width, height);

        {
            let buffer = frame2.buffer.get_mut().unwrap();
            let mut data = buffer.map_writable().unwrap();

            let map = frame.buffer.map_readable().unwrap();
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
    }

    // Copy the `other` frame into `self`, with the top left at dx/dy
    pub fn copy(&mut self, other: &Frame, dx: usize, dy: usize) {
        let mut data = self.buffer.get_mut().unwrap().map_writable().unwrap();

        let map = other.buffer.map_readable().unwrap();
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
    pub fn write_to(&self, filename: &str) {
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
                        ("width", &(self.width as i32)),
                        ("height", &(self.height as i32)),
                    ],
                ),
            )
            .unwrap();

        let jpegenc = gst::ElementFactory::make("jpegenc", None).unwrap();
        let filesink = gst::ElementFactory::make("filesink", None).unwrap();
        filesink.set_property("location", &filename).unwrap();

        let pipeline = gst::Pipeline::new(None);
        pipeline
            .add_many(&[&src, &capsfilter, &jpegenc, &filesink])
            .unwrap();
        gst::Element::link_many(&[&src, &capsfilter, &jpegenc, &filesink]).unwrap();

        let appsrc = src
            .clone()
            .dynamic_cast::<gst_app::AppSrc>()
            .expect("Sink element is expected to be an appsrc!");
        appsrc.set_property_format(gst::Format::Time);
        appsrc.set_property_block(true);

        pipeline
            .set_state(gst::State::Playing)
            .into_result()
            .unwrap();

        appsrc
            .push_buffer(self.buffer.copy_deep().unwrap())
            .into_result()
            .unwrap();

        appsrc.end_of_stream().into_result().unwrap();

        let bus = pipeline.get_bus().unwrap();

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

        pipeline.set_state(gst::State::Null).into_result().unwrap();
    }
}
