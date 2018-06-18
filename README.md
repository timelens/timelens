# Timelens

**Timelens** creates visual timelines from video files. For a general introduction, please visit <https://timelens.io>.

## Building from source

Timelens is written in the Rust programming language, so you'll need a working [Rust installation](https://www.rust-lang.org). You'll probably want to run these commands:

    $ curl -f https://sh.rustup.rs > rustup-init.sh
    $ ./rustup-init.sh
    $ source ~/.cargo/env

Then, compiling Timelens is as easy as this:

    $ git clone https://github.com/timelens/timelens
    $ cd timelens
    $ cargo build --release

This will create the binary `target/release/timelens`:

    $ ./target/release/timelens my_favorite_movie.mp4

To run the test suite, run `cargo test`.

## Contributing

Development of *Timelens* happens on GitHub. Please report any bugs or ideas to the [issue tracker](https://github.com/timelens/timelens/issues). To contribute code, fork the repository and submit a pull request.

You can also help by packaging the software for your favorite operating system, or writing an integration for your favorite video player.

Please note that this project is released with a [Contributor Code of Conduct](CODE_OF_CONDUCT.md). By participating in this project you agree to abide by its terms.

## License: GPLv2+

See [LICENSE.md](LICENSE.md) for details.
