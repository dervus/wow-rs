use std::ops::Deref;
use std::fmt;
use rgb::{RGB8, RGBA8};

pub struct Blob<T>(pub Vec<T>);

impl<T: Sized> From<Vec<T>> for Blob<T> {
    fn from(a: Vec<T>) -> Blob<T> {
        Blob(a)
    }
}

impl<T> Deref for Blob<T> {
    type Target = Vec<T>;

    fn deref(&self) -> &Vec<T> {
        &self.0
    }
}

macro_rules! impl_debug {
    ($($T:ty;)+) => {
        $(
            impl fmt::Debug for Blob<$T> {
                fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
                    write!(f, "Blob [{}; {}]", stringify!($T), self.0.len())
                }
            }
        )*
    }
}

impl_debug! {
    u8;
    (u8, u8);
    (u8, u8, u8);
    (u8, u8, u8, u8);
    f32;
    (f32, f32);
    (f32, f32, f32);
    (f32, f32, f32, f32);
    RGB8;
    RGBA8;
}