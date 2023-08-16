use palette::color_difference::EuclideanDistance;
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
}

fn find_similar_color(color: Srgba, palette: &Vec<Color>) -> Color {
    let mut best_distance = f32::MAX;
    let mut best_color = Color::default();

    for palette_color in palette {
        let distance = color.distance_squared(*palette_color.srgba);

        if distance < best_distance {
            best_distance = distance;
            best_color = *palette_color;
        }
    }

    best_color
}
