#[macro_use]
extern crate log;
extern crate simplelog;
extern crate wow;

use std::path::Path;
use std::sync::Arc;

fn main() {
    simplelog::TermLogger::init(simplelog::LevelFilter::Trace, simplelog::Config::default())
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

        let ext = wow::reader::split_resource_name(file).2.to_lowercase();

        let reader = Arc::new(wow::reader::fs::FsResourceReader::new(dir));

        match ext.as_str() {
            ".blp" => match wow::blp::load(reader.clone(), file) {
                Ok(image) => {
                    info!("{:?} loaded successfully", path);
                    println!("{:#?}", &image);
                }
                Err(e) => {
                    error!("Failed to load {:?}; Cause: {}", path, &e);
                }
            }
            ".adt" => match wow::adt::MapTile::load(reader.clone(), file, None) {
                Ok(_) => {
                    info!("{:?} loaded successfully", path);
                }
                Err(e) => {
                    error!("Failed to load {:?}; Cause: {}", path, &e);
                }
            }
            ".wmo" => match wow::wmo::load(reader.clone(), file) {
                Ok(wmo) => {
                    info!("{:?} loaded successfully", path);
                    let mut loaded_groups = Vec::new();

                    for group_info in &wmo.groups {
                        match group_info.load(reader.clone()) {
                            Ok(wmo_group) => {
                                info!("{:?} loaded successfully", &group_info.resource_key);
                                loaded_groups.push(wmo_group);
                            }
                            Err(error) => {
                                error!("Failed to load {:?}; Cause: {}", &group_info.resource_key, error);
                            }
                        }
                    }

                    println!("{:#?}", &wmo);
                    for group in loaded_groups { println!("{:#?}", &group) }
                }
                Err(e) => {
                    error!("Failed to load {:?}; Cause: {}", path, &e);
                }
            }
            _ => error!("Unknown file extension: {}", &ext)
        }
    }
}
