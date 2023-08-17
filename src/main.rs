use std::io::Cursor;
use std::vec::IntoIter;
use image::{DynamicImage, GenericImageView, ImageResult, Pixel};
use image::io::Reader;
use palette::color_difference::Wcag21RelativeContrast;
use palette::Srgba;

fn main() {
    println!("Hello, world!");
    let test = Srgba::<u8>::new(127, 127, 127, 127);
    let test2 = Srgba::<u8>::new(25, 25, 25, 0);
    println!("{}", distance_squared(test, test2));
}

#[derive(Copy, Clone)]
struct Color {
    id: u8,
    srgba: Srgba<u8>
}

impl Default for Color {
    fn default() -> Self {
        Color { id: 0, srgba: Srgba::new(0, 0, 0, 0) }
    }
}

struct ImageColors {
    colors: Vec<Srgba<u8>>,
    height: u32
}

impl ImageColors {
    fn color(&self, x: u32, y: u32) -> Srgba<u8> {
        self.colors[(y * self.height + x) as usize]
    }
}

impl IntoIterator for ImageColors {
    type Item = Srgba<u8>;
    type IntoIter = IntoIter<Srgba<u8>>;

    fn into_iter(self) -> Self::IntoIter {
        self.colors.into_iter()
    }
}

impl From<DynamicImage> for ImageColors {
    fn from(image: DynamicImage) -> Self {
        let width = image.width();
        let height = image.height();
        let mut colors = Vec::with_capacity((width * height) as usize);

        for x in 0..width {
            for y in 0..height {
                let color = image.get_pixel(x, y).to_rgba();
                let channels = color.channels();
                let red = channels[0];
                let green = channels[1];
                let blue = channels[2];
                let alpha = channels[3];

                colors.push(Srgba::new(red, green, blue, alpha));
            }
        }

        ImageColors { colors, height }
    }
}

fn decode_image_from_path(path: &str) -> ImageResult<DynamicImage> {
    Reader::open(path)?.decode()
}

fn decode_image_from_bytes(raw_data: &str) -> ImageResult<DynamicImage> {
    Reader::new(Cursor::new(raw_data))
        .with_guessed_format()
        .expect("Cursor IO never fails")
        .decode()
}

fn change_palette(original_colors: Vec<Srgba<u8>>, palette: &Vec<Color>) -> Vec<Color> {
    original_colors.into_iter().map(|color| find_similar_color(color, palette)).collect()
}

fn find_similar_color(color: Srgba<u8>, palette: &Vec<Color>) -> Color {
    let mut best_distance = u32::MAX;
    let mut best_color = Color::default();

    for palette_color in palette {
        let distance = distance_squared(color, palette_color.srgba);

        if distance < best_distance {
            best_distance = distance;
            best_color = *palette_color;
        }
    }

    best_color
}

fn distance_squared(color1: Srgba<u8>, color2: Srgba<u8>) -> u32 {

    // u8 squared -> u16 needed, u16 x 4 -> u32 needed
    // Ex: 255^2 * 4 = 260100
    component_distance_squared(color1.red, color2.red)
        + component_distance_squared(color1.green, color2.green)
        + component_distance_squared(color1.blue, color2.blue)
        + component_distance_squared(color1.alpha, color2.alpha)

}

fn component_distance_squared(component1: u8, component2: u8) -> u32 {
    let distance = component1.abs_diff(component2) as u32;
    distance * distance
}

fn range_relative_luminance(colors: &[Color]) -> Vec<f32> {
    let (min_luma, max_luma) = colors.iter()
        .map(|color| {
            let srgba_f32: Srgba<f32> = color.srgba.into_format();
            srgba_f32.relative_luminance().luma
        })
        .fold((0.0f32, 1.0f32), |(min, max), luma| (min.min(luma), max.max(luma)));

    let range = max_luma - min_luma;

    colors.iter()
        .map(|color| {
            let srgba_f32: Srgba<f32> = color.srgba.into_format();
            srgba_f32.relative_luminance().luma
        })
        .map(|luma| (luma - min_luma) / range)
        .collect()
}
