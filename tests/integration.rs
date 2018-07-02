#![feature(extern_prelude)]

extern crate assert_cmd;
extern crate assert_fs;
extern crate gstreamer as gst;
extern crate predicates;

#[cfg(test)]
mod integration {
    use assert_cmd::prelude::*;
    use assert_fs::prelude::*;
    use gst;
    use gst::prelude::*;
    use predicates::prelude::*;
    use std::env;
    use std::path::Path;
    use std::process::Command;

    #[test]
    fn basics() {
        fail("");
        ok("--help");
        ok("--version");
        ok("-V");
    }

    #[test]
    fn input_file() {
        fail("");
        fail("''");
        fail("' '");
        fail("does_not_exist.123");
        fail(".");
        fail("..");
        fail("/");
        fail("\\");

        // ok_with_file("-w 1");
        ok_with_file("-w 16 -h 16");
    }

    #[test]
    fn size() {
        assert_test_file_exists();

        fail_with_file("-w");
        fail_with_file("-h");

        fail_with_file("-w foo");
        fail_with_file("-h foo");

        fail_with_file("-w -100");
        fail_with_file("-h -100");

        fail_with_file("-w 100.0");
        fail_with_file("-h 100.0");

        fail_with_file("-w 0");
        fail_with_file("-h 0");

        fail_with_file("-w ''");
        fail_with_file("-h ''");

        ok_with_file("-w 16 -h 16");
        ok_with_file("-h 16");
        ok_with_file("-w 160");
    }

    #[test]
    fn output() {
        assert_test_file_exists();
        let filename = test_file_name();

        fail_with_file("-w 16 --timeline");
        fail_with_file("-w 16 --timeline .");
        fail_with_file("-w 16 --timeline ..");
        fail_with_file("-w 16 --timeline /");

        fail_with_file(&format!("-w 16 --timeline {}", filename));
    }

    #[test]
    fn vtt() {
        let tmp_dir = assert_fs::TempDir::new().unwrap();
        let vtt_file = tmp_dir.child("test.vtt");
        let thumbnails_file = tmp_dir.child("test.jpg");

        ok_with_file(&format!(
            "--vtt {} --thumbnails {} -w 16 -h 16",
            &vtt_file.path().to_str().unwrap(),
            &thumbnails_file.path().to_str().unwrap()
        ));

        thumbnails_file.assert(predicate::path::is_file());
        vtt_file.assert(predicate::path::is_file());
        vtt_file.assert(
            predicate::str::contains("test.jpg?xywh=0,0,120,90")
                .from_utf8()
                .from_file_path(),
        );
        vtt_file.assert(
            predicate::str::contains("nope")
                .not()
                .from_utf8()
                .from_file_path(),
        );
    }

    fn test_file_name() -> String {
        let mut filename = env::temp_dir();
        filename.push("timelens_test.mkv");
        String::from(filename.as_path().to_str().unwrap())
    }

    fn create_test_file() {
        let filename = test_file_name();

        gst::init().unwrap();
        let pipeline = gst::parse_launch(&format!("videotestsrc num-buffers=50 ! videoconvert ! vp8enc ! matroskamux ! filesink location={}", &filename)).unwrap();

        pipeline
            .set_state(gst::State::Playing)
            .into_result()
            .unwrap();

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

    fn assert_test_file_exists() {
        let filename = test_file_name();

        if !Path::new(&filename).exists() {
            create_test_file();
        }

        assert!(Path::new(&filename).exists());
    }

    fn ok(args_string: &str) {
        let args: Vec<&str> = args_string.split(' ').collect();
        Command::main_binary()
            .unwrap()
            .args(&args)
            .assert()
            .success();
    }

    fn fail(args_string: &str) {
        let args: Vec<&str> = args_string.split(' ').collect();
        Command::main_binary()
            .unwrap()
            .args(&args)
            .assert()
            .failure();
    }

    fn ok_with_file(args_string: &str) {
        let filename = test_file_name();
        let mut args: Vec<&str> = args_string.split(' ').collect();
        args.push(&filename);
        Command::main_binary()
            .unwrap()
            .args(&args)
            .assert()
            .success();
    }

    fn fail_with_file(args_string: &str) {
        let filename = test_file_name();
        let mut args: Vec<&str> = args_string.split(' ').collect();
        args.push(&filename);
        Command::main_binary()
            .unwrap()
            .args(&args)
            .assert()
            .failure();
    }
}
