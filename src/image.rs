use image::{GenericImageView, Pixel, Rgba};
use crate::{Image, RawColor};

fn convert_color(color:  Rgba<u8>) -> RawColor {
    let channels = color.channels();

    let red = channels[0];
    let green = channels[1];
    let blue = channels[2];
    let alpha = channels[3];

    RawColor::new(red, green, blue, alpha)
}

pub struct View<'a, I> {
    image: &'a I,
    l: u32,
    w: u32,
    length: u32,
    width: u32
}

impl<I: GenericImageView<Pixel=Rgba<u8>>> Image for View<'_, I> {
    type SubImage = Self;

    fn pixel(&self, l: u32, w: u32) -> RawColor {
        convert_color(self.image.get_pixel(self.l + l, self.w + w))
    }

    fn length(&self) -> u32 {
        self.length
    }

    fn width(&self) -> u32 {
        self.width
    }

    fn view(&self, l: u32, w: u32, length: u32, width: u32) -> Self::SubImage {
        View {
            image: &self.image,
            l: self.l + l,
            w: self.w + w,
            length,
            width,
        }
    }
}

impl<'a, I: GenericImageView<Pixel=Rgba<u8>>> Image for &'a I {
    type SubImage = View<'a, I>;

    fn pixel(&self, l: u32, w: u32) -> RawColor {
        convert_color(self.get_pixel(l, w))
    }

    fn length(&self) -> u32 {
        I::width(&self)
    }

    fn width(&self) -> u32 {
        I::height(&self)
    }

    fn view(&self, l: u32, w: u32, length: u32, width: u32) -> Self::SubImage {
        View {
            image: &self,
            l,
            w,
            length,
            width
        }
    }
}
