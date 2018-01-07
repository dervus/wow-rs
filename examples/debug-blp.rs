#[macro_use]
extern crate log;
extern crate simplelog;
extern crate wow;

use std::path::Path;
use std::sync::Arc;

fn main() {
    simplelog::TermLogger::init(simplelog::LogLevelFilter::Trace, simplelog::Config::default())
        .expect("failed to initialize logger");

    for arg in std::env::args() {
        let path = Path::new(&arg);

        let dir = path
            .parent()
            .expect("target path must have parent directory");

        let file = path
            .file_name()
            .expect("target path must be a file")
            .to_str()
            .expect("file name must be valid UTF-8");

        let reader = Arc::new(wow::reader::fs::FsResourceReader::new(dir));

        match wow::blp::load(reader, file) {
            Ok(image) => {
                info!("{:?} loaded successfully", path);
                println!("{:#?}", &image);
            },
            Err(e) => {
                error!("Failed to load {:?}; Cause: {}", path, &e);
            }
        }
    }
}