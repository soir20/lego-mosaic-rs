use image::{GenericImageView, Pixel, Rgba};
use crate::{Image, RawColor};

impl<I: GenericImageView<Pixel=Rgba<u8>>> Image for I {
    fn pixel(&self, l: u32, w: u32) -> RawColor {
        let color = self.get_pixel(l, w);
        let channels = color.channels();

        let red = channels[0];
        let green = channels[1];
        let blue = channels[2];
        let alpha = channels[3];

        RawColor::new(red, green, blue, alpha)
    }

    fn length(&self) -> u32 {
        I::width(self)
    }

    fn width(&self) -> u32 {
        I::height(self)
    }
}
