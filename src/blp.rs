use std::fmt;
use std::io::{self, Read, Seek, SeekFrom};
use std::sync::Arc;
use byteorder::{ReadBytesExt, LittleEndian as LE};
use rgb::{RGB8, RGBA8};
use misc::Blob;
use read_ext::ReadExt;
use reader::ResourceReader;

const PALETTE_SIZE: usize = 256;
#[allow(dead_code)]
pub struct Palette([RGB8; PALETTE_SIZE]);

impl fmt::Debug for Palette {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        fmt::Debug::fmt(&&self.0[..], f)
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct IndexedPixels {
    pub indexes: Blob<u8>,
    pub alpha_values: Option<Blob<u8>>,
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum Compression {
    DXT1,
    DXT3,
    DXT5,
}

#[allow(dead_code)]
#[derive(Debug)]
pub enum ImageData {
    TrueColor {
        mipmaps: Vec<Blob<RGBA8>>,
    },
    Indexed {
        palette: Palette,
        full_alpha: bool,
        mipmaps: Vec<IndexedPixels>,
    },
    Compressed {
        compression: Compression,
        mipmaps: Vec<Blob<u8>>,
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct Image {
    pub height: u32,
    pub width: u32,
    pub data: ImageData,
}

#[allow(dead_code)]
pub fn load(reader: Arc<ResourceReader>, name: &str) -> io::Result<Image> {
    let mut input = reader.open(name)?;

    let mut magic = [0u8; 4];
    input.read_exact(&mut magic)?;

    if magic != [b'B', b'L', b'P', b'2'] {
        return Err(io_error!(InvalidData, "file header isn't BLP2"));
    }

    let_read! { input =>
        u32 version;
        u8 encoding;
        u8 alpha_depth;
        u8 preferred_format;
        u8 has_mipmaps;
        u32 width;
        u32 height;
    }

    if version != 1 {
        return Err(io_error!(InvalidData, "unsupported BLP version: {}", version));
    }

    if alpha_depth != 0 && alpha_depth != 1 && alpha_depth != 8 {
        warn!("Trying to load BLP with unsupported alpha depth: {}", alpha_depth);
    }

    let mipmap_blocks: Vec<(u32, u32)> = {
        let mut offsets = [0u32; 16];
        let mut sizes = [0u32; 16];

        for index in 0..16 {
            let value = input.read_u32::<LE>()?;
            offsets[index] = value;
        }

        for index in 0..16 {
            let value = input.read_u32::<LE>()?;
            sizes[index] = value;
        }

        offsets.iter()
            .take_while(|x| **x != 0)
            .map(|x| *x)
            .zip(sizes.iter().take_while(|x| **x != 0).map(|x| *x))
            .collect()
    };
    trace!("mipmap_blocks = {:?}", &mipmap_blocks);

    let data = match encoding {
        //
        // uncompressed, indexed
        //
        1 => {
            let mut mipmaps = Vec::with_capacity(mipmap_blocks.len());

            let mut palette = [RGB8::default(); PALETTE_SIZE];
            for index in 0..PALETTE_SIZE {
                let color = input.read_u8tuple4()?;
                palette[index] = RGB8 {
                    b: color.0,
                    g: color.1,
                    r: color.2
                };
            }

            for (offset, size) in mipmap_blocks {
                input.seek(SeekFrom::Start(offset as u64))?;
                let mut block = input.take(size as u64);

                let mut indexes = block.read_vec((height * width) as usize)?;

                let alpha_values = match alpha_depth {
                    8 => {
                        Some(block.read_vec((height * width) as usize)?)
                    },
                    1 => {
                        let mut alpha_values = Vec::with_capacity((height * width) as usize);

                        for _ in 0..(((height * width) as f32 / 8.0).ceil() as u32) {
                            let byte = block.read_u8()?;
                            for bit in 0..8 {
                                if byte & (1 << bit) != 0 {
                                    alpha_values.push(0xFF);
                                } else {
                                    alpha_values.push(0x00);
                                }
                            }
                        }
                        Some(alpha_values)
                    },
                    _ => None
                };

                input = block.into_inner();

                mipmaps.push(IndexedPixels {
                    indexes: indexes.into(),
                    alpha_values: alpha_values.map(|v| v.into())
                });
            }

            ImageData::Indexed {
                palette: Palette(palette),
                full_alpha: alpha_depth > 1,
                mipmaps
            }
        }
        //
        // compressed, DXTC
        //
        2 => {
            let compression = {
                if alpha_depth == 8 && preferred_format == 7 {
                    Compression::DXT5
                } else if alpha_depth == 8 || alpha_depth == 4 {
                    Compression::DXT3
                } else {
                    Compression::DXT1
                }
            };

            let mut mipmaps = Vec::with_capacity(mipmap_blocks.len());

            for (offset, size) in mipmap_blocks {
                trace!("reading {} bytes at {} offset", size, offset);
                input.seek(SeekFrom::Start(offset as u64))?;
                mipmaps.push(Blob(input.read_vec(size as usize)?));
            }

            ImageData::Compressed { compression, mipmaps }
        }
        //
        // uncompressed, truecolor
        //
        3 => {
            let mut mipmaps = Vec::with_capacity(mipmap_blocks.len());

            for (offset, _) in mipmap_blocks {
                input.seek(SeekFrom::Start(offset as u64))?;
                let mut pixels = Vec::with_capacity((height * width) as usize);

                for _ in 0..height {
                    for _ in 0..width {
                        let color = input.read_u8tuple4()?;
                        pixels.push(RGBA8 {
                            b: color.0,
                            g: color.1,
                            r: color.2,
                            a: color.3
                        });
                    }
                }

                mipmaps.push(Blob(pixels));
            }

            ImageData::TrueColor { mipmaps }
        }
        _ => return Err(io_error!(InvalidData, "unsupported encoding id: {}", encoding))
    };

    Ok(Image {
        height,
        width,
        data
    })
}