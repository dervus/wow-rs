use std::collections::HashMap;
use std::str::from_utf8;
use std::io::{self, Read, Cursor};
use byteorder::{ReadBytesExt, LittleEndian as LE};

type ChunkData = Vec<u8>;
type ChunkMap = HashMap<String, Vec<ChunkData>>;

fn read_chunks_into_map<R>(input: &mut R, legion_m2: bool) -> io::Result<ChunkMap>
    where R: Read + ReadBytesExt
{
    let mut result = ChunkMap::new();
    let mut token_buffer = [0u8; 4];

    loop {
        match input.read(&mut token_buffer) {
            Err(e) => {
                return Err(e)
            },
            Ok(bytes) if bytes == 0 => {
                return Ok(result)
            },
            Ok(bytes) if bytes < 4 => {
                return Err(io_error!(UnexpectedEof, "incomplete chunk token"))
            },
            Ok(_) => {
                // seems like wow uses u32 constants to encode chunk tokens. legion's m2 files
                // encode those using big-endian, while all other files use little-endian.
                // so for all but legion m2, we reverse byte order to get left-to-right token names.
                if !legion_m2 { token_buffer.reverse(); }

                let token = from_utf8(&token_buffer)
                    .map(|s| s.to_string())
                    .map_err(|e| io_error!(InvalidData, e))?;

                let size = input.read_u32::<LE>()?;
                let mut data = Vec::with_capacity(size as usize);
                input.take(size as u64).read_to_end(&mut data)?;

                trace!("found chunk: {} ({} bytes)", &token, size);

                if let Some(existing) = result.get_mut(&token) {
                    existing.push(data);
                    continue;
                }
                // else
                result.insert(token, vec![data]);
            }
        }
    }
}

#[allow(dead_code)]
pub fn read_chunks<R, F>(input: &mut R, legion_m2: bool, f: F) -> io::Result<()>
    where R: Read + ReadBytesExt,
          F: Fn(&str, Cursor<&Vec<u8>>) -> io::Result<()>
{
    let chunk_map = read_chunks_into_map(input, legion_m2)?;

    for (token, chunks) in chunk_map {
        for chunk in chunks {
            let cursor = Cursor::new(&chunk);
            f(&token, cursor)?;
        }
    }
    Ok(())
}

pub struct Chunked {
    inner: ChunkMap,
}

impl Chunked {
    pub fn read<R: Read>(input: &mut R) -> io::Result<Self> {
        Ok(Self { inner: read_chunks_into_map(input, false)? })
    }

    #[allow(dead_code)]
    pub fn read_legion_m2<R: Read>(input: &mut R) -> io::Result<Self> {
        Ok(Self { inner: read_chunks_into_map(input, true)? })
    }

    pub fn get(&self, token: &str) -> io::Result<&ChunkData> {
        trace!("looking up chunk: {}", token);
        self.inner
            .get(token)
            .and_then(|x| x.first())
            .ok_or(io_error!(InvalidData, "chunk not found: {}", token))
    }

    pub fn get_all(&self, token: &str) -> io::Result<&Vec<ChunkData>> {
        trace!("looking up chunk: {}", token);
        self.inner
            .get(token)
            .ok_or(io_error!(InvalidData, "chunk not found: {}", token))
    }

    pub fn cursor(&self, token: &str) -> io::Result<Cursor<&ChunkData>> {
        self.get(token)
            .map(|data| Cursor::new(data))
    }

    #[allow(dead_code)]
    pub fn cursor_all(&self, token: &str) -> io::Result<Vec<Cursor<&ChunkData>>> {
        self.get_all(token)
            .map(|chunks| chunks.iter().map(|data| Cursor::new(data)).collect())
    }
}

macro_rules! let_chunk {
    ($chunks:ident $token:expr => $($kind:ident $name:ident;)*) => {
        let token = $token;
        let mut reader = $chunks.cursor(&token)?;
        let_read! { reader => $($kind $name;)* }
        {
            let len = reader.get_ref().len();
            let pos = reader.position() as usize;
            if pos < len {
                warn!("{} chunk data has {} unread bytes", &token, len - pos);
            } else {
                trace!("chunk reading done");
            }
        }
        drop(reader);
    }
}