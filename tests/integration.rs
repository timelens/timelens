extern crate assert_cli;

#[cfg(test)]
mod integration {
    use assert_cli;

    fn ok(args_string: &str) {
        let args: Vec<&str> = args_string.split(' ').collect();
        assert_cli::Assert::main_binary().with_args(&args).unwrap();
    }

    fn fail(args_string: &str) {
        let args: Vec<&str> = args_string.split(' ').collect();
        assert_cli::Assert::main_binary()
            .with_args(&args)
            .fails()
            .unwrap();
    }

    #[test]
    fn invalid_input_file() {
        fail("''");
        fail("does_not_exist.123");
        fail(".");
        fail("..");
        fail("/");
    }

    #[test]
    fn size() {
        ok("-w 16 -h 16");

        fail("-w");
        fail("-h");

        fail("-w foo");
        fail("-h foo");

        fail("-w -100");
        fail("-h -100");

        fail("-w 100.0");
        fail("-h 100.0");

        fail("-w 0");
        fail("-h 0");

        fail("-w ''");
        fail("-h ''");
    }
}
