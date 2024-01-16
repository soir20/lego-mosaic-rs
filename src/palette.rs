use kd_tree::{KdPoint, KdTree};
use palette::{IntoColor, LinSrgba, Srgba};
use palette::color_difference::{Ciede2000, HyAb};
use crate::{Color, Palette, RawColor};

// ====================
// PUBLIC STRUCTS
// ====================

#[derive(Clone, PartialEq, Debug, Default)]
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

#[derive(Clone, PartialEq, Debug, Default)]
pub struct HyAbPalette<C> {
    palette: Vec<Lab<C>>
}

impl<C: Color> HyAbPalette<C> {
    pub fn new(palette: &[C]) -> Self {
        HyAbPalette {
            palette: lab_palette(palette)
        }
    }
}

impl<C: Color> Palette<C> for HyAbPalette<C> {
    fn nearest(&self, color: RawColor) -> Option<C> {
        lab_nearest(&self.palette, color, |given_color, candidate| given_color.hybrid_distance(candidate))
    }
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct Ciede2000Palette<C> {
    palette: Vec<Lab<C>>
}

impl<C: Color> Ciede2000Palette<C> {
    pub fn new(palette: &[C]) -> Self {
        Ciede2000Palette {
            palette: lab_palette(palette)
        }
    }
}

impl<C: Color> Palette<C> for Ciede2000Palette<C> {
    fn nearest(&self, color: RawColor) -> Option<C> {
        lab_nearest(&self.palette, color, |given_color, candidate| given_color.difference(candidate))
    }
}

// ====================
// PRIVATE STRUCTS
// ====================

#[derive(Clone, PartialEq, Debug, Default)]
struct EuclideanDistanceKdPoint<C>(C, [f64; 4]);

impl<C: Color> KdPoint for EuclideanDistanceKdPoint<C> {

    // Use f64 to allow for multiplication, subtraction without overflow
    type Scalar = f64;
    type Dim = typenum::U4;

    fn at(&self, i: usize) -> Self::Scalar {
        self.1[i]
    }
}

#[derive(Clone, PartialEq, Debug, Default)]
struct Lab<C> {
    original: C,
    linear_alpha: f32,
    lab: palette::Lab
}

// ====================
// PRIVATE FUNCTIONS
// ====================

fn to_linear(color: RawColor) -> [f64; 4] {
    let linear: LinSrgba<f64> = Srgba::new(color.red, color.green, color.blue, color.alpha).into_linear();
    linear.into()
}

fn to_lab(color: RawColor) -> palette::Lab {
    let linear_color: LinSrgba<f32> = Srgba::new(
        color.red,
        color.green,
        color.blue,
        color.alpha
    ).into_linear();

    linear_color.into_color()
}

fn lab_palette<C: Color>(palette: &[C]) -> Vec<Lab<C>> {
    palette.iter().map(|&original| {
        let srgba = original.into();
        Lab {
            original,
            linear_alpha: to_linear(srgba)[3] as f32,
            lab: to_lab(srgba)
        }
    }).collect()
}

fn lab_nearest<C: Color>(palette: &[Lab<C>], color: RawColor, diff_fn: impl Fn(palette::Lab, palette::Lab) -> f32) -> Option<C> {
    let linear_alpha = to_linear(color)[3] as f32;
    let lab_color = to_lab(color);

    palette.iter()
        .fold((None, f32::INFINITY), |(best_color, best_distance), candidate| {

            /* Lab does not consider the alpha channel, so weight it similarly to Euclidean distance.
               The maximum Lab distance is 100, so the alpha distance is clamped to a scale of 0-100. */
            let alpha_distance = 0.25f32 * ((linear_alpha - candidate.linear_alpha).abs() * 100f32);
            let distance = 0.75f32 * diff_fn(lab_color, candidate.lab) + alpha_distance;

            if distance < best_distance {
                (Some(candidate), distance)
            } else {
                (best_color, best_distance)
            }
        })
        .0
        .map(|color| color.original)
}

//noinspection DuplicatedCode
#[cfg(test)]
mod tests {
    use crate::{Palette, RawColor};
    use crate::palette::{Ciede2000Palette, EuclideanDistancePalette, HyAbPalette};
    use crate::tests::TestColor;

    #[test]
    fn test_empty_euclidean() {
        let palette: EuclideanDistancePalette<TestColor> = EuclideanDistancePalette::new(&[]);
        let nearest = palette.nearest(RawColor { red: 2, green: 86, blue: 105, alpha: 203 });
        assert!(nearest.is_none());
    }

    #[test]
    fn test_euclidean_finds_red() {
        let palette = EuclideanDistancePalette::new(&[
            TestColor::new(128, 127, 127, 127),
            TestColor::new(127, 128, 127, 127),
            TestColor::new(127, 127, 128, 127),
            TestColor::new(127, 127, 127, 128)
        ]);
        let nearest = palette.nearest(RawColor { red: 255, green: 0, blue: 0, alpha: 0 }).unwrap();
        assert_eq!(TestColor::new(128, 127, 127, 127), nearest);
    }

    #[test]
    fn test_euclidean_finds_green() {
        let palette = EuclideanDistancePalette::new(&[
            TestColor::new(128, 127, 127, 127),
            TestColor::new(127, 128, 127, 127),
            TestColor::new(127, 127, 128, 127),
            TestColor::new(127, 127, 127, 128)
        ]);
        let nearest = palette.nearest(RawColor { red: 0, green: 255, blue: 0, alpha: 0 }).unwrap();
        assert_eq!(TestColor::new(127, 128, 127, 127), nearest);
    }

    #[test]
    fn test_euclidean_finds_blue() {
        let palette = EuclideanDistancePalette::new(&[
            TestColor::new(128, 127, 127, 127),
            TestColor::new(127, 128, 127, 127),
            TestColor::new(127, 127, 128, 127),
            TestColor::new(127, 127, 127, 128)
        ]);
        let nearest = palette.nearest(RawColor { red: 0, green: 0, blue: 255, alpha: 0 }).unwrap();
        assert_eq!(TestColor::new(127, 127, 128, 127), nearest);
    }

    #[test]
    fn test_euclidean_finds_alpha() {
        let palette = EuclideanDistancePalette::new(&[
            TestColor::new(128, 127, 127, 127),
            TestColor::new(127, 128, 127, 127),
            TestColor::new(127, 127, 128, 127),
            TestColor::new(127, 127, 127, 128)
        ]);
        let nearest = palette.nearest(RawColor { red: 0, green: 0, blue: 0, alpha: 255 }).unwrap();
        assert_eq!(TestColor::new(127, 127, 127, 128), nearest);
    }

    #[test]
    fn test_euclidean_finds_one_when_equidistant() {
        let palette = EuclideanDistancePalette::new(&[
            TestColor::new(128, 127, 127, 127),
            TestColor::new(127, 128, 127, 127),
            TestColor::new(127, 127, 128, 127),
            TestColor::new(127, 127, 127, 128)
        ]);
        let nearest = palette.nearest(RawColor { red: 255, green: 255, blue: 255, alpha: 255 });
        assert!(nearest.is_some());
    }

    #[test]
    fn test_empty_hyab() {
        let palette: HyAbPalette<TestColor> = HyAbPalette::new(&[]);
        let nearest = palette.nearest(RawColor { red: 2, green: 86, blue: 105, alpha: 203 });
        assert!(nearest.is_none());
    }

    #[test]
    fn test_hyab_finds_red() {
        let palette = HyAbPalette::new(&[
            TestColor::new(128, 127, 127, 127),
            TestColor::new(127, 128, 127, 127),
            TestColor::new(127, 127, 128, 127),
            TestColor::new(127, 127, 127, 128)
        ]);
        let nearest = palette.nearest(RawColor { red: 255, green: 0, blue: 0, alpha: 0 }).unwrap();
        assert_eq!(TestColor::new(128, 127, 127, 127), nearest);
    }

    #[test]
    fn test_hyab_finds_green() {
        let palette = HyAbPalette::new(&[
            TestColor::new(128, 127, 127, 127),
            TestColor::new(127, 128, 127, 127),
            TestColor::new(127, 127, 128, 127),
            TestColor::new(127, 127, 127, 128)
        ]);
        let nearest = palette.nearest(RawColor { red: 0, green: 255, blue: 0, alpha: 0 }).unwrap();
        assert_eq!(TestColor::new(127, 128, 127, 127), nearest);
    }

    #[test]
    fn test_hyab_finds_blue() {
        let palette = HyAbPalette::new(&[
            TestColor::new(128, 127, 127, 127),
            TestColor::new(127, 128, 127, 127),
            TestColor::new(127, 127, 128, 127),
            TestColor::new(127, 127, 127, 128)
        ]);
        let nearest = palette.nearest(RawColor { red: 0, green: 0, blue: 255, alpha: 0 }).unwrap();
        assert_eq!(TestColor::new(127, 127, 128, 127), nearest);
    }

    #[test]
    fn test_hyab_finds_alpha() {
        let palette = HyAbPalette::new(&[
            TestColor::new(128, 127, 127, 127),
            TestColor::new(127, 128, 127, 127),
            TestColor::new(127, 127, 128, 127),
            TestColor::new(127, 127, 127, 128)
        ]);
        let nearest = palette.nearest(RawColor { red: 0, green: 0, blue: 0, alpha: 255 }).unwrap();
        assert_eq!(TestColor::new(127, 127, 127, 128), nearest);
    }

    #[test]
    fn test_hyab_finds_one_when_equidistant() {
        let palette = HyAbPalette::new(&[
            TestColor::new(128, 127, 127, 127),
            TestColor::new(127, 128, 127, 127),
            TestColor::new(127, 127, 128, 127),
            TestColor::new(127, 127, 127, 128)
        ]);
        let nearest = palette.nearest(RawColor { red: 255, green: 255, blue: 255, alpha: 255 });
        assert!(nearest.is_some());
    }

    #[test]
    fn test_empty_ciede() {
        let palette: Ciede2000Palette<TestColor> = Ciede2000Palette::new(&[]);
        let nearest = palette.nearest(RawColor { red: 2, green: 86, blue: 105, alpha: 203 });
        assert!(nearest.is_none());
    }

    #[test]
    fn test_ciede_finds_red() {
        let palette = Ciede2000Palette::new(&[
            TestColor::new(128, 127, 127, 127),
            TestColor::new(127, 128, 127, 127),
            TestColor::new(127, 127, 128, 127),
            TestColor::new(127, 127, 127, 128)
        ]);
        let nearest = palette.nearest(RawColor { red: 255, green: 0, blue: 0, alpha: 0 }).unwrap();
        assert_eq!(TestColor::new(128, 127, 127, 127), nearest);
    }

    #[test]
    fn test_ciede_finds_green() {
        let palette = Ciede2000Palette::new(&[
            TestColor::new(128, 127, 127, 127),
            TestColor::new(127, 128, 127, 127),
            TestColor::new(127, 127, 128, 127),
            TestColor::new(127, 127, 127, 128)
        ]);
        let nearest = palette.nearest(RawColor { red: 0, green: 255, blue: 0, alpha: 0 }).unwrap();
        assert_eq!(TestColor::new(127, 128, 127, 127), nearest);
    }

    #[test]
    fn test_ciede_finds_blue() {
        let palette = Ciede2000Palette::new(&[
            TestColor::new(128, 127, 127, 127),
            TestColor::new(127, 128, 127, 127),
            TestColor::new(127, 127, 128, 127),
            TestColor::new(127, 127, 127, 128)
        ]);
        let nearest = palette.nearest(RawColor { red: 0, green: 0, blue: 255, alpha: 0 }).unwrap();
        assert_eq!(TestColor::new(127, 127, 128, 127), nearest);
    }

    #[test]
    fn test_ciede_finds_alpha() {
        let palette = Ciede2000Palette::new(&[
            TestColor::new(128, 127, 127, 127),
            TestColor::new(127, 128, 127, 127),
            TestColor::new(127, 127, 128, 127),
            TestColor::new(127, 127, 127, 128)
        ]);
        let nearest = palette.nearest(RawColor { red: 0, green: 0, blue: 0, alpha: 255 }).unwrap();
        assert_eq!(TestColor::new(127, 127, 127, 128), nearest);
    }

    #[test]
    fn test_ciede_finds_one_when_equidistant() {
        let palette = Ciede2000Palette::new(&[
            TestColor::new(128, 127, 127, 127),
            TestColor::new(127, 128, 127, 127),
            TestColor::new(127, 127, 128, 127),
            TestColor::new(127, 127, 127, 128)
        ]);
        let nearest = palette.nearest(RawColor { red: 255, green: 255, blue: 255, alpha: 255 });
        assert!(nearest.is_some());
    }

}
