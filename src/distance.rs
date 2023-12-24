use kd_tree::{KdPoint, KdTree};
use palette::{IntoColor, LinSrgba, Srgba};
use palette::color_difference::HyAb;
use crate::{Color, Palette, RawColor};

// ====================
// PUBLIC STRUCTS
// ====================

pub struct EuclideanDistancePalette<C: Color> {
    tree: KdTree<EuclideanDistanceKdPoint<C>>
}

impl<C: Color> EuclideanDistancePalette<C> {
    pub fn new(palette: &[C]) -> Self {
        let mapped_palette = palette.iter()
            .map(|&color| {
                let srgba = color.into();
                EuclideanDistanceKdPoint(color, to_linear(srgba))
            }).collect();
        EuclideanDistancePalette { tree: KdTree::build_by_ordered_float(mapped_palette) }
    }
}

impl<C: Color> Palette<C> for EuclideanDistancePalette<C> {
    fn nearest(&self, color: RawColor) -> Option<C> {
        let components = to_linear(color);
        self.tree.nearest(&components).map(|result| result.item.0)
    }
}

pub struct HyAbPalette<C> {
    palette: Vec<Lab<C>>
}

impl<C: Color> HyAbPalette<C> {
    pub fn new(palette: &[C]) -> Self {
        HyAbPalette {
            palette: palette.iter().map(|&original| {
                let srgba = original.into();
                Lab {
                    original,
                    linear_alpha: to_linear(srgba)[3] as f32,
                    lab: to_lab(srgba)
                }
            }).collect()
        }
    }
}

impl<C: Color> Palette<C> for HyAbPalette<C> {
    fn nearest(&self, color: RawColor) -> Option<C> {
        let linear_alpha = to_linear(color)[3] as f32;
        let lab_color = to_lab(color);

        self.palette.iter()
            .fold((None, f32::INFINITY), |(best_color, best_distance), candidate| {

                /* HyAb does not consider the alpha channel, so weight it similarly to Euclidean distance.
                   The maximum HyAb distance is 100, so the alpha distance is clamped to a scale of 0-100. */
                let alpha_distance = 0.25f32 * ((linear_alpha - candidate.linear_alpha).abs() * 100f32);
                let distance = 0.75f32 * lab_color.hybrid_distance(candidate.lab) + alpha_distance;

                if distance < best_distance {
                    (Some(candidate), distance)
                } else {
                    (best_color, best_distance)
                }
            })
            .0
            .map(|color| color.original)
    }
}

// ====================
// PRIVATE STRUCTS
// ====================

struct EuclideanDistanceKdPoint<C>(C, [f64; 4]);

impl<C: Color> KdPoint for EuclideanDistanceKdPoint<C> {

    // Use f64 to allow for multiplication, subtraction without overflow
    type Scalar = f64;
    type Dim = typenum::U4;

    fn at(&self, i: usize) -> Self::Scalar {
        self.1[i]
    }
}

struct Lab<C> {
    original: C,
    linear_alpha: f32,
    lab: palette::Lab
}

// ====================
// PRIVATE FUNCTIONS
// ====================

fn to_linear(color: RawColor) -> [f64; 4] {
    let linear: LinSrgba<f64> = Srgba::new(*color.red(), *color.green(), *color.blue(), *color.alpha()).into_linear();
    linear.into()
}

fn to_lab(color: RawColor) -> palette::Lab {
    let linear_color: LinSrgba<f32> = Srgba::new(
        *color.red(),
        *color.green(),
        *color.blue(),
        *color.alpha()
    ).into_linear();

    linear_color.into_color()
}
