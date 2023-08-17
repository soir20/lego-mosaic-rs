use palette::color_difference::{EuclideanDistance, Wcag21RelativeContrast};
use palette::Srgba;

#[derive(Copy, Clone)]
struct Color {
    id: u8,
    srgba: Srgba
}

impl Default for Color {
    fn default() -> Self {
        Color { id: 0, srgba: Srgba::new(0.0, 0.0, 0.0, 0.0) }
    }
}

fn main() {
    println!("Hello, world!");
    let test: Srgba = Srgba::new(0.5, 0.5, 0.5, 0.5);
    let test2: Srgba = Srgba::new(0.1, 0.1, 0.1, 0.0);
    println!("{}", distance_squared(test, test2));
}

fn find_similar_color(color: Srgba, palette: &Vec<Color>) -> Color {
    let mut best_distance = f32::MAX;
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

fn distance_squared(color1: Srgba, color2: Srgba) -> f32 {
    let alpha_distance = color1.alpha - color2.alpha;
    let alpha_distance_squared = alpha_distance * alpha_distance;
    color1.distance_squared(*color2) + alpha_distance_squared
}

fn range_relative_luminance(colors: &Vec<Color>) -> Vec<f32> {
    let min_luma = colors.iter()
        .map(|color| color.srgba.relative_luminance().luma)
        .min_by(|a, b| a.partial_cmp(b).expect("Luminance was NaN"))
        .unwrap_or(0.0);
    let max_luma = colors.iter()
        .map(|color| color.srgba.relative_luminance().luma)
        .max_by(|a, b| a.partial_cmp(b).expect("Luminance was NaN"))
        .unwrap_or(1.0);

    let range = max_luma - min_luma;

    colors.iter().map(|color| color.srgba.relative_luminance().luma)
        .map(|luma| (luma - min_luma) / range)
        .collect()
}
