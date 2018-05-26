use std::str::from_utf8;
use std::iter::Iterator;
use std::io::{self, Cursor};
use byteorder::LE;
use read_ext::ReadExt;

pub struct Chunk {
    pub token: String,
    pub data: Vec<u8>,
}

impl Chunk {
    pub fn new(token: String, data: Vec<u8>) -> Self {
        Chunk { token, data }
    }

    pub fn cursor(&self) -> Cursor<&[u8]> {
        Cursor::new(&self.data)
    }
}

pub struct Chunked<R: ReadExt> {
    reader: R,
    legion_m2: bool,
}

impl<R: ReadExt> Chunked<R> {
    pub fn new(reader: R) -> Self {
        Chunked { reader, legion_m2: false }
    }

    pub fn new_legion_m2(reader: R) -> Self {
        Chunked { reader, legion_m2: true }
    }

    fn read_chunk(&mut self) -> io::Result<Option<Chunk>> {
        let mut token_buffer = [0u8; 4];

        match self.reader.read(&mut token_buffer) {
            Err(e) => {
                Err(e)
            },
            Ok(bytes) if bytes == 0 => {
                Ok(None)
            },
            Ok(bytes) if bytes < 4 => {
                // FIXME: WTF
                Ok(None)
                //Err(io_error!(UnexpectedEof, "incomplete chunk token"))
            },
            Ok(_) => {
                // seems like wow uses u32 constants to encode chunk tokens. legion's m2 files
                // encode those using big-endian, while all other files use little-endian.
                // so for all but legion m2, we reverse byte order to get left-to-right token names.
                if !self.legion_m2 { token_buffer.reverse(); }

                let token = from_utf8(&token_buffer)
                    .map(|s| s.to_owned())
                    .map_err(|e| io_error!(InvalidData, e))?;

                let size = self.reader.read_u32::<LE>()?;

                trace!("found chunk: {} ({} bytes)", &token, size);

                let mut data = self.reader.read_vec(size as usize)?;

                Ok(Some(Chunk::new(token, data)))
            }
        }
    }
}

impl<R: ReadExt> Iterator for Chunked<R> {
    type Item = io::Result<Chunk>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.read_chunk() {
            Ok(None) => None,
            Ok(Some(item)) => Some(Ok(item)),
            Err(e) => Some(Err(e)),
        }
    }
}
