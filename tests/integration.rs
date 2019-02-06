extern crate assert_cmd;
extern crate assert_fs;
extern crate gstreamer as gst;
extern crate predicates;

#[cfg(test)]
mod integration {
    use crate::gst;
    use crate::gst::prelude::*;
    use assert_cmd::prelude::*;
    use assert_fs::prelude::*;
    use predicates::prelude::*;
    use std::env;
    use std::path::Path;
    use std::process::Command;

    #[test]
    fn basics() {
        fail("");
        fail("--fail");

        ok("--help");
        ok("--version");
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

        ok_with_file("--");
    }

    #[test]
    fn size() {
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

        fail_with_file("-w 1");
        fail_with_file("-h 1");

        fail_with_file("-w 15");
        fail_with_file("-h 15");

        fail_with_file("-w 16 -h 10001");
        fail_with_file("-w 10001 -h 16");

        fail_with_file("-w 159");

        ok_with_file("-w 160");
        ok_with_file("-h 16");
        ok_with_file("-w 16 -h 16");
        ok_with_file("-w 16 -h 1000");
        ok_with_file("-w 1000 -h 16");

        ok_with_file("-w 1000");
        ok_with_file("-h 1000");
        ok_with_file("-w 1000 -h 1000");

        ok_with_file("-w 10000");
        ok_with_file("-w 16 -h 10000");
    }

    #[test]
    fn timeline_file() {
        let filename = test_file_name();

        fail_with_file("--timeline");
        fail_with_file("--timeline .");
        fail_with_file("--timeline ..");
        fail_with_file("--timeline /");
        fail_with_file("--timeline nope.txt");

        fail_with_file(&format!("--timeline {}", filename));
        ok_with_file(&format!("--timeline {}.different.jpg", filename));
    }

    #[test]
    fn thumbnail_height() {
        let tmp_dir =
            assert_fs::TempDir::new().expect("Could not make new tempdir for thumbnail height");
        let vtt_file = tmp_dir.child("test.vtt");

        fail_with_file("-H 90");
        fail_with_file(&format!(
            "-- thumbnails {} -H nope",
            &vtt_file
                .path()
                .to_str()
                .expect("Could not convert VTT filename to str")
        ));
        fail_with_file(&format!(
            "--thumbnails {} -H -100",
            &vtt_file
                .path()
                .to_str()
                .expect("Could not convert VTT filename to str")
        ));
        fail_with_file(&format!(
            "--thumbnails {} -H 0",
            &vtt_file
                .path()
                .to_str()
                .expect("Could not convert VTT filename to str")
        ));
        fail_with_file(&format!(
            "--thumbnails {} -H ''",
            &vtt_file
                .path()
                .to_str()
                .expect("Could not convert VTT filename to str")
        ));

        fail_with_file(&format!(
            "--thumbnails {} -H 15",
            &vtt_file
                .path()
                .to_str()
                .expect("Could not convert VTT filename to str")
        ));

        ok_with_file(&format!(
            "--thumbnails {} -H 16",
            &vtt_file
                .path()
                .to_str()
                .expect("Could not convert VTT filename to str")
        ));
        ok_with_file(&format!(
            "--thumbnails {} -H 100",
            &vtt_file
                .path()
                .to_str()
                .expect("Could not convert VTT filename to str")
        ));

        fail_with_file(&format!(
            "--thumbnails {} -H 10001",
            &vtt_file
                .path()
                .to_str()
                .expect("Could not convert VTT filename to str")
        ));
    }

    #[test]
    fn thumbnails() {
        let tmp_dir = assert_fs::TempDir::new().expect("Could not make tempdir for thumbnails");
        let vtt_file = tmp_dir.child("test.vtt");
        let thumbnails_file = tmp_dir.child("test-01.jpg");

        fail_with_file("--thumbnails nope.jpg");

        ok_with_file(&format!(
            "--thumbnails {} -w 1000 -h 1000 -H 120",
            &vtt_file
                .path()
                .to_str()
                .expect("Could not convert VTT filename to str"),
        ));

        thumbnails_file.assert(predicate::path::is_file());
        vtt_file.assert(predicate::path::is_file());
        vtt_file.assert(
            predicate::str::contains("test")
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
        String::from(
            filename
                .as_path()
                .to_str()
                .expect("Could not convert test file name to str"),
        )
    }

    fn create_test_file() {
        let filename = test_file_name();

        gst::init().expect("Could not initialize GStreamer");
        let pipeline = gst::parse_launch(&format!("videotestsrc num-buffers=20 ! videoconvert ! vp8enc ! matroskamux ! filesink location={}", &filename)).expect("Could not parse-launch pipeline for creating test file");

        pipeline
            .set_state(gst::State::Playing)
            .into_result()
            .expect("Could not start creating pipeline");

        let bus = pipeline
            .get_bus()
            .expect("Could not get bus from creating pipeline");

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
            .expect("Could not stop creating pipeline");
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
        Command::cargo_bin(env!("CARGO_PKG_NAME"))
            .expect("Could not set up binary")
            .args(&args)
            .assert()
            .success();
    }

    fn fail(args_string: &str) {
        let args: Vec<&str> = args_string.split(' ').collect();
        Command::cargo_bin(env!("CARGO_PKG_NAME"))
            .expect("Could not set up binary, part 2")
            .args(&args)
            .assert()
            .failure();
    }

    fn ok_with_file(args_string: &str) {
        assert_test_file_exists();

        let filename = test_file_name();
        let mut args: Vec<&str> = args_string.split(' ').collect();
        args.push(&filename);
        Command::cargo_bin(env!("CARGO_PKG_NAME"))
            .expect("Could not set up binary, part 3")
            .args(&args)
            .assert()
            .success();
    }

    fn fail_with_file(args_string: &str) {
        assert_test_file_exists();

        let filename = test_file_name();
        let mut args: Vec<&str> = args_string.split(' ').collect();
        args.push(&filename);
        Command::cargo_bin(env!("CARGO_PKG_NAME"))
            .expect("Could not set up binary, part 4")
            .args(&args)
            .assert()
            .failure();
    }
}
