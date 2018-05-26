use std::io::{self, Read, Seek};

pub trait ResourceHandle : Read + Seek {}
impl<R: Read + Seek> ResourceHandle for R {}

pub trait ResourceReader {
    fn exists(&self, name: &str) -> io::Result<bool>;
    fn open(&self, name: &str) -> io::Result<Box<ResourceHandle>>;
}

pub fn split_resource_name(input: &str) -> (&str, &str, &str) {
    let mut ext_pos = input.len();
    let mut file_pos = 0;

    for (index, ch) in input.chars().rev().enumerate() {
        let pos = input.len() - (index + 1);

        match ch {
            '.' => {
                ext_pos = pos;
            }
            '/' | '\\' => {
                file_pos = pos + 1;
                break;
            }
            _ => {}
        }
    }

    let dir = &input[..file_pos];
    let file = &input[file_pos..ext_pos];
    let ext = &input[ext_pos..];

    (dir, file, ext)
}

pub mod fs;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_resource_name() {
        assert_eq!(split_resource_name(""), ("", "", ""));

        assert_eq!(split_resource_name("World\\wmo\\Azeroth\\Stormwind.wmo"),   ("World\\wmo\\Azeroth\\",   "Stormwind", ".wmo"));
        assert_eq!(split_resource_name("World\\wmo\\Azeroth\\\\Stormwind.wmo"), ("World\\wmo\\Azeroth\\\\", "Stormwind", ".wmo"));

        assert_eq!(split_resource_name("World/wmo/Azeroth/Stormwind.wmo"),  ("World/wmo/Azeroth/",  "Stormwind", ".wmo"));
        assert_eq!(split_resource_name("World/wmo/Azeroth//Stormwind.wmo"), ("World/wmo/Azeroth//", "Stormwind", ".wmo"));

        assert_eq!(split_resource_name("World/Stormwind.wmo.xml"), ("World/", "Stormwind", ".wmo.xml"));
        assert_eq!(split_resource_name("World/Stormwind.wmo"),     ("World/", "Stormwind", ".wmo"));
        assert_eq!(split_resource_name("World/.wtf"),              ("World/", "",          ".wtf"));
        assert_eq!(split_resource_name("World/Stormwind."),        ("World/", "Stormwind", "."));
        assert_eq!(split_resource_name("World/Stormwind"),         ("World/", "Stormwind", ""));
        assert_eq!(split_resource_name("World/"),                  ("World/", "",          ""));
        assert_eq!(split_resource_name("Stormwind.wmo.xml"),       ("",       "Stormwind", ".wmo.xml"));
        assert_eq!(split_resource_name("Stormwind.wmo"),           ("",       "Stormwind", ".wmo"));
        assert_eq!(split_resource_name("Stormwind"),               ("",       "Stormwind", ""));
        assert_eq!(split_resource_name(".wtf"),                    ("",       "",          ".wtf"));
    }
}
