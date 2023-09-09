use image::{DynamicImage, GenericImageView, Pixel};
use palette::color_difference::Wcag21RelativeContrast;
use palette::Srgba;

pub type Color = Srgba<u8>;
pub struct SinglePieceFlatMosaic {
    colors: Pixels<Color>
}

impl SinglePieceFlatMosaic {
    pub fn from_image(image: DynamicImage, palette: &[Color]) -> SinglePieceFlatMosaic {
        let raw_pixels: Pixels<Color> = image.into();
        SinglePieceFlatMosaic { colors: raw_pixels.with_palette(palette) }
    }

    pub fn color(&self, x: u32, y: u32) -> Color {
        self.colors.value(x, y)
    }

    pub fn make_3d(self, layers: u16, darker_areas_taller: bool) -> SinglePiece3dMosaic {
        let bump_map = self.colors.bump_map(layers, darker_areas_taller);
        SinglePiece3dMosaic { colors: self.colors, bump_map }
    }
}

pub struct SinglePiece3dMosaic {
    colors: Pixels<Color>,
    bump_map: Pixels<u16>
}

impl SinglePiece3dMosaic {

    pub fn color(&self, x: u32, y: u32) -> Color {
        self.colors.value(x, y)
    }

    pub fn height(&self, x: u32, y: u32) -> u16 {
        self.bump_map.value(x, y)
    }

}

struct Pixels<T> {
    values_by_row: Vec<T>,
    width: u32
}

impl<T: Copy> Pixels<T> {
    fn value(&self, x: u32, y: u32) -> T {
        self.values_by_row[(y * self.width + x) as usize]
    }
}

impl From<DynamicImage> for Pixels<Srgba<u8>> {
    fn from(image: DynamicImage) -> Self {
        let width = image.width();
        let height = image.height();
        let mut colors = Vec::with_capacity((width * height) as usize);

        for y in 0..height {
            for x in 0..width {
                let color = image.get_pixel(x, y).to_rgba();
                let channels = color.channels();
                let red = channels[0];
                let green = channels[1];
                let blue = channels[2];
                let alpha = channels[3];

                colors.push(Srgba::new(red, green, blue, alpha));
            }
        }

        Pixels { values_by_row: colors, width }
    }
}

impl Pixels<Srgba<u8>> {
    pub fn with_palette(self, palette: &[Color]) -> Pixels<Color> {
        let new_colors = self.values_by_row.into_iter()
            .map(|color| Self::find_similar_color(color, palette))
            .collect();
        Pixels { values_by_row: new_colors, width: self.width }
    }

    fn find_similar_color(color: Srgba<u8>, palette: &[Color]) -> Color {
        let mut best_distance = u32::MAX;
        let mut best_color = Color::default();

        for &palette_color in palette {
            let distance = Self::distance_squared(color, palette_color);

            if distance < best_distance {
                best_distance = distance;
                best_color = palette_color;
            }
        }

        best_color
    }

    fn distance_squared(color1: Srgba<u8>, color2: Srgba<u8>) -> u32 {

        // u8 squared -> u16 needed, u16 x 4 -> u32 needed
        // Ex: 255^2 * 4 = 260100
        Self::component_distance_squared(color1.red, color2.red)
            + Self::component_distance_squared(color1.green, color2.green)
            + Self::component_distance_squared(color1.blue, color2.blue)
            + Self::component_distance_squared(color1.alpha, color2.alpha)

    }

    fn component_distance_squared(component1: u8, component2: u8) -> u32 {
        let distance = component1.abs_diff(component2) as u32;
        distance * distance
    }
}

type BumpMap = Pixels<u16>;
impl Pixels<Color> {
    pub fn bump_map(&self, layers: u16, flip: bool) -> BumpMap {
        if layers == 0 {
            return BumpMap { values_by_row: Vec::new(), width: 0 }
        }

        let (min_luma, max_luma) = self.values_by_row.iter()
            .map(|color| {
                let srgba_f32: Srgba<f32> = color.into_format();
                srgba_f32.relative_luminance().luma
            })
            .fold((0.0f32, 1.0f32), |(min, max), luma| (min.min(luma), max.max(luma)));

        let range = max_luma - min_luma;
        let max_layer_index = layers - 1;

        let heights_by_row = self.values_by_row.iter()
            .map(|color| {
                let srgba_f32: Srgba<f32> = color.into_format();
                srgba_f32.relative_luminance().luma
            })
            .map(|luma| (luma - min_luma) / range)
            .map(|range_rel_luma| if flip { 1.0 - range_rel_luma } else { range_rel_luma })
            /* Layers must be u16 because the max integer a 32-bit float can represent exactly
               is 2^24 + 1 (more than u16::MAX but less than u32::MAX). */
            .map(|range_rel_luma| (range_rel_luma * max_layer_index as f32).round() as u16 + 1)
            .collect();

        BumpMap { values_by_row: heights_by_row, width: self.width }
    }
}
