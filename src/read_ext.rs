use std::collections::BTreeMap;
use std::io::{self, Read};
use byteorder::{ReadBytesExt, ByteOrder};

macro_rules! io_error {
    ($id:ident, $err:expr) => (io::Error::new(io::ErrorKind::$id, $err));
    ($id:ident, $fmt:expr, $($arg:expr),+) => (io::Error::new(io::ErrorKind::$id, format!($fmt, $($arg),+)));
    ($err:expr) => (io_error!(Other, $err));
    ($err:expr, $($arg:expr),+) => (io_error!(Other, $err, $($arg),+));
}

macro_rules! read {
    ($r:ident, u8) =>  ($r.read_u8());
    ($r:ident, u16) => {{use byteorder; $r.read_u16::<byteorder::LittleEndian>()}};
    ($r:ident, u32) => {{use byteorder; $r.read_u32::<byteorder::LittleEndian>()}};
    ($r:ident, u64) => {{use byteorder; $r.read_u64::<byteorder::LittleEndian>()}};
    ($r:ident, i8) =>  ($r.read_i8());
    ($r:ident, i16) => {{use byteorder; $r.read_i16::<byteorder::LittleEndian>()}};
    ($r:ident, i32) => {{use byteorder; $r.read_i32::<byteorder::LittleEndian>()}};
    ($r:ident, i64) => {{use byteorder; $r.read_i64::<byteorder::LittleEndian>()}};
    ($r:ident, f32) => {{use byteorder; $r.read_f32::<byteorder::LittleEndian>()}};
    ($r:ident, f64) => {{use byteorder; $r.read_f64::<byteorder::LittleEndian>()}};
    ($r:ident, u8tuple4) => ($r.read_u8tuple4());
    ($r:ident, f32tuple2) => {{use byteorder; $r.read_f32tuple2::<byteorder::LittleEndian>()}};
    ($r:ident, f32tuple3) => {{use byteorder; $r.read_f32tuple3::<byteorder::LittleEndian>()}};
    ($r:ident, f32tuple4) => {{use byteorder; $r.read_f32tuple4::<byteorder::LittleEndian>()}};
}

macro_rules! let_read {
    ($reader:ident => $($kind:ident $name:ident;)+) => {
        $(
            let $name = read!($reader, $kind)?;
            trace!("{} = {:?}", stringify!($name), $name);
        )*
    };
}

pub trait ReadExt : Read + ReadBytesExt {
    fn read_vec_into(&mut self, size: usize, target: &mut Vec<u8>) -> io::Result<()> {
        let bytes_read = self.take(size as u64).read_to_end(target)?;

        if bytes_read < size {
            return Err(io_error!(UnexpectedEof, "cannot read {} more bytes from input", bytes_read));
        }

        Ok(())
    }

    fn read_vec(&mut self, size: usize) -> io::Result<Vec<u8>> {
        let mut result = Vec::with_capacity(size);
        self.read_vec_into(size, &mut result)?;
        Ok(result)
    }

    fn read_cstring(&mut self) -> io::Result<String> {
        let mut result = Vec::new();
        loop {
            let byte = self.read_u8()?;
            if byte == 0 { break }
            result.push(byte);
        }
        String::from_utf8(result).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    fn read_cstring_table_into(&mut self, target: &mut BTreeMap<u32, String>) -> io::Result<()> {
        let mut string_buffer = Vec::new();
        let mut index = 0;
        let mut string_id = -1;
        let mut at_string = false;
        let mut done: bool;

        loop {
            let mut read_buffer = [0u8; 1];
            done = self.read(&mut read_buffer)? == 0;
            let byte = if done { 0u8 } else { read_buffer[0] };

            if byte != 0 {
                if !at_string {
                    at_string = true;
                    string_id = index;
                }
                string_buffer.push(byte);
            } else if !string_buffer.is_empty() {
                at_string = false;

                let string = String::from_utf8(string_buffer.clone())
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
                string_buffer.clear();

                trace!("found string: [{}] '{}'", string_id, &string);
                target.insert(string_id as u32, string);
                string_id = -1;
            }

            if done { break }
            index += 1;
        }
        Ok(())
    }

    fn read_cstring_table(&mut self) -> io::Result<BTreeMap<u32, String>> {
        let mut result = BTreeMap::new();
        self.read_cstring_table_into(&mut result)?;
        Ok(result)
    }

    fn read_u8tuple4(&mut self) -> io::Result<(u8, u8, u8, u8)> {
        let _1 = self.read_u8()?;
        let _2 = self.read_u8()?;
        let _3 = self.read_u8()?;
        let _4 = self.read_u8()?;
        Ok((_1, _2, _3, _4))
    }

    fn read_f32tuple2<T: ByteOrder>(&mut self) -> io::Result<(f32, f32)> {
        let _1 = self.read_f32::<T>()?;
        let _2 = self.read_f32::<T>()?;
        Ok((_1, _2))
    }

    fn read_f32tuple3<T: ByteOrder>(&mut self) -> io::Result<(f32, f32, f32)> {
        let _1 = self.read_f32::<T>()?;
        let _2 = self.read_f32::<T>()?;
        let _3 = self.read_f32::<T>()?;
        Ok((_1, _2, _3))
    }

    fn read_f32tuple4<T: ByteOrder>(&mut self) -> io::Result<(f32, f32, f32, f32)> {
        let _1 = self.read_f32::<T>()?;
        let _2 = self.read_f32::<T>()?;
        let _3 = self.read_f32::<T>()?;
        let _4 = self.read_f32::<T>()?;
        Ok((_1, _2, _3, _4))
    }
}
impl<R: Read + ReadBytesExt> ReadExt for R {}