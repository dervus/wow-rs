use std::collections::BTreeMap;
use std::io::{self, Read};
use byteorder::{ReadBytesExt, ByteOrder};

macro_rules! io_error {
    ($id:ident, $err:expr) => (io::Error::new(io::ErrorKind::$id, $err));
    ($id:ident, $fmt:expr, $($arg:expr),+) => (io::Error::new(io::ErrorKind::$id, format!($fmt, $($arg),+)));
    ($err:expr) => (io_error!(Other, $err));
    ($err:expr, $($arg:expr),+) => (io_error!(Other, $err, $($arg),+));
}

pub trait ReadExt : Read + ReadBytesExt {
    fn read_vec_into(&mut self, size: usize, target: &mut Vec<u8>) -> io::Result<()> {
        let bytes_read = self.take(size as u64).read_to_end(target)?;

        if bytes_read < size {
            return Err(io_error!(UnexpectedEof, "cannot read {} out of {} bytes from input", bytes_read, size));
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

    fn read_cstring_table_with<F>(&mut self, mut f: F) -> io::Result<()>
    where
        F: FnMut(u32, String)
    {
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
                f(string_id as u32, string);
                string_id = -1;
            }

            if done { break }
            index += 1;
        }
        Ok(())
    }

    fn read_cstring_table_into(&mut self, target: &mut BTreeMap<u32, String>) -> io::Result<()> {
        self.read_cstring_table_with(|index, string| {
            target.insert(index, string);
        })
    }

    fn read_cstring_table(&mut self) -> io::Result<BTreeMap<u32, String>> {
        let mut result = BTreeMap::new();
        self.read_cstring_table_into(&mut result)?;
        Ok(result)
    }

    fn read_cstring_array_into(&mut self, target: &mut Vec<String>) -> io::Result<()> {
        self.read_cstring_table_with(|_, string| {
            target.push(string);
        })
    }

    fn read_cstring_array(&mut self) -> io::Result<Vec<String>> {
        let mut result = Vec::new();
        self.read_cstring_array_into(&mut result)?;
        Ok(result)
    }

    #[inline]
    fn read_u8tuple4(&mut self) -> io::Result<(u8, u8, u8, u8)> {
        let _1 = self.read_u8()?;
        let _2 = self.read_u8()?;
        let _3 = self.read_u8()?;
        let _4 = self.read_u8()?;
        Ok((_1, _2, _3, _4))
    }

    #[inline]
    fn read_f32tuple2<T: ByteOrder>(&mut self) -> io::Result<(f32, f32)> {
        let _1 = self.read_f32::<T>()?;
        let _2 = self.read_f32::<T>()?;
        Ok((_1, _2))
    }

    #[inline]
    fn read_f32tuple3<T: ByteOrder>(&mut self) -> io::Result<(f32, f32, f32)> {
        let _1 = self.read_f32::<T>()?;
        let _2 = self.read_f32::<T>()?;
        let _3 = self.read_f32::<T>()?;
        Ok((_1, _2, _3))
    }

    #[inline]
    fn read_f32tuple4<T: ByteOrder>(&mut self) -> io::Result<(f32, f32, f32, f32)> {
        let _1 = self.read_f32::<T>()?;
        let _2 = self.read_f32::<T>()?;
        let _3 = self.read_f32::<T>()?;
        let _4 = self.read_f32::<T>()?;
        Ok((_1, _2, _3, _4))
    }
}
impl<R: Read + ReadBytesExt> ReadExt for R {}

pub trait FromRead: Sized {
    fn from_read<O: ByteOrder>(reader: &mut Read) -> io::Result<Self>;
}

impl FromRead for i8 {
    fn from_read<O: ByteOrder>(reader: &mut Read) -> io::Result<Self> {
        reader.read_i8()
    }
}

impl FromRead for u8 {
    fn from_read<O: ByteOrder>(reader: &mut Read) -> io::Result<Self> {
        reader.read_u8()
    }
}


impl FromRead for (u8, u8, u8, u8) {
    fn from_read<O: ByteOrder>(mut reader: &mut Read) -> io::Result<Self> {
        reader.read_u8tuple4()
    }
}

impl FromRead for (f32, f32) {
    fn from_read<O: ByteOrder>(mut reader: &mut Read) -> io::Result<Self> {
        reader.read_f32tuple2::<O>()
    }
}

impl FromRead for (f32, f32, f32) {
    fn from_read<O: ByteOrder>(mut reader: &mut Read) -> io::Result<Self> {
        reader.read_f32tuple3::<O>()
    }
}

impl FromRead for (f32, f32, f32, f32) {
    fn from_read<O: ByteOrder>(mut reader: &mut Read) -> io::Result<Self> {
        reader.read_f32tuple4::<O>()
    }
}

macro_rules! impl_from_read {
    ($method:ident, $fortype:ty) => {
        impl ::read_ext::FromRead for $fortype {
            fn from_read<O: ::byteorder::ByteOrder>(reader: &mut ::std::io::Read) -> ::std::io::Result<Self> {
                reader.$method::<O>()
            }
        }
    };
}
impl_from_read!(read_i16, i16);
impl_from_read!(read_i32, i32);
impl_from_read!(read_i64, i64);
impl_from_read!(read_u16, u16);
impl_from_read!(read_u32, u32);
impl_from_read!(read_u64, u64);
impl_from_read!(read_f32, f32);
impl_from_read!(read_f64, f64);

macro_rules! impl_bitflags_from_read {
    ($method:ident, $fortype:ident) => {
        impl ::read_ext::FromRead for $fortype {
            fn from_read<O: ::byteorder::ByteOrder>(reader: &mut ::std::io::Read) -> ::std::io::Result<Self> {
                let bits = reader.$method::<O>()?;
                Ok($fortype::from_bits_truncate(bits))
            }
        }
    }
}

macro_rules! let_read {
    ($byteorder:ty | $reader:ident => $( $fieldname:ident : $fieldtype:ty ; )+ ) => {
        $(
            let $fieldname = <$fieldtype as ::read_ext::FromRead>::from_read::<$byteorder>(&mut $reader)?;
            trace!("{} = {:?}", stringify!($fieldname), &$fieldname);
        )*
    }
}
