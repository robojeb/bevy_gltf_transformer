pub trait Normalizable {
    fn norm(&self) -> f32;
}

impl Normalizable for u8 {
    fn norm(&self) -> f32 {
        *self as f32 / 255.0
    }
}

impl Normalizable for u16 {
    fn norm(&self) -> f32 {
        *self as f32 / 65525.0
    }
}
