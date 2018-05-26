use std::io::{self, Read};
use read_ext::ReadExt;

const ALPHAMAP_SIDE: usize = 64;
const ALPHAMAP_SIZE: usize = ALPHAMAP_SIDE ^ 2;

#[derive(Clone, Debug)]
pub struct AlphaMap {
    is_u4: bool,
    data: Vec<u8>,
}

impl AlphaMap {
    pub fn read_raw<R: ReadExt>(reader: &mut R, is_u4: bool) -> io::Result<Self> {
        let len = if is_u4 { ALPHAMAP_SIZE / 2 } else { ALPHAMAP_SIZE };
        let data = reader.read_vec(len)?;

        Ok(AlphaMap { is_u4, data })
    }

    pub fn read_compressed<R: ReadExt>(reader: &mut R) -> io::Result<Self> {
        let mut data = Vec::with_capacity(ALPHAMAP_SIZE);

        while data.len() < ALPHAMAP_SIZE {
            let byte = reader.read_u8()?;
            let fill_mode = byte & 0x80 != 0;
            let count = byte & 0x7f;

            if fill_mode {
                let pattern = reader.read_u8()?;
                for _ in 0..count { data.push(pattern) }
            } else {
                reader.take(count as u64).read_to_end(&mut data)?;
            }
        }

        if data.len() == ALPHAMAP_SIZE {
            Ok(AlphaMap { is_u4: false, data })
        } else {
            Err(io_error!(InvalidData, "invalid compressed alpha map format"))
        }
    }

    pub fn is_u4(&self) -> bool {
        self.is_u4
    }

    pub fn is_u8(&self) -> bool {
        !self.is_u4
    }

    pub fn as_slice(&self) -> &[u8] {
        self.data.as_slice()
    }

    pub fn get(&self, index: usize) -> u8 {
        let value = self.data[index / 2];

        if self.is_u4 {
            if value % 2 == 0 {
                value & 0b00001111
            } else {
                (value & 0b11110000) >> 4
            }
        } else {
            value
        }
    }

    pub fn get_f32(&self, index: usize) -> f32 {
        let value = self.get(index) as f32;

        if self.is_u4 { value / 16.0 }
            else { value / 256.0 }
    }

    pub fn values(&self) -> AlphaValues {
        AlphaValues::new(self)
    }
}

#[derive(Clone, Debug)]
pub struct AlphaValue {
    pub row: usize,
    pub column: usize,
    pub value: f32,
}

pub struct AlphaValues<'a> {
    alpha_map: &'a AlphaMap,
    index: usize,
}

impl<'a> AlphaValues<'a> {
    pub fn new(alpha_map: &'a AlphaMap) -> Self {
        AlphaValues { alpha_map, index: 0 }
    }
}

impl<'a> Iterator for AlphaValues<'a> {
    type Item = AlphaValue;

    fn next(&mut self) -> Option<Self::Item> {
        if self.index < 64 * 64 {
            let value = self.alpha_map.get_f32(self.index);
            let row = self.index / 64;
            let column = self.index % 64;

            self.index += 1;
            Some(AlphaValue { row, column, value })
        } else {
            None
        }
    }
}
