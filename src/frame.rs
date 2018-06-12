extern crate gstreamer as gst;
extern crate gstreamer_app as gst_app;
use frame::gst::prelude::*;

pub struct Frame {
    pub buffer: gst::Buffer,
    pub width: usize,
    pub height: usize,
    pub pts: Option<f32>,
}

impl Frame {
    pub fn new(width: usize, height: usize) -> Frame {
        let buffer = gst::Buffer::with_size(width * height * 4).unwrap();
        Frame {
            buffer,
            width,
            height,
            pts: None,
        }
    }

    pub fn write_to(&self, filename: &String) {
        let (pipeline, src) =
            build_output_pipeline(self.width as i32, self.height as i32, filename);

        pipeline
            .set_state(gst::State::Playing)
            .into_result()
            .unwrap();

        src.push_buffer(self.buffer.copy_deep().unwrap())
            .into_result()
            .unwrap();

        src.end_of_stream().into_result().unwrap();

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

// build a pipeline that writes the first frame pushed into the Appsrc to a JPEG file
fn build_output_pipeline(
    width: i32,
    height: i32,
    filename: &String,
) -> (gst::Pipeline, gst_app::AppSrc) {
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
                    ("width", &width),
                    ("height", &height),
                ],
            ),
        )
        .unwrap();

    let jpegenc = gst::ElementFactory::make("jpegenc", None).unwrap();
    let filesink = gst::ElementFactory::make("filesink", None).unwrap();
    filesink.set_property("location", &filename).unwrap();

    let output_pipeline = gst::Pipeline::new(None);
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
