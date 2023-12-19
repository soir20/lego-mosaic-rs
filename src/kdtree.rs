// ====================
// PUBLIC STRUCTS
// ====================

use kd_tree::{KdPoint, KdTree};
use crate::{Color, Palette, RawColor};

pub struct EuclideanDistancePalette<C: Color> {
    tree: KdTree<ColorKdPoint<C>>
}

impl<C: Color> EuclideanDistancePalette<C> {
    pub fn new(palette: &[C]) -> Self {
        let mapped_palette = palette.into_iter().map(|&color| ColorKdPoint(color)).collect();
        EuclideanDistancePalette { tree: KdTree::build(mapped_palette) }
    }
}

impl<C: Color> Palette<C> for EuclideanDistancePalette<C> {
    fn nearest(&self, color: RawColor) -> Option<C> {
        let components = <RawColor as Into<[u8; 4]>>::into(color).map(i64::from);
        self.tree.nearest(&components).map(|result| result.item.0)
    }
}

// ====================
// PRIVATE STRUCTS
// ====================

struct ColorKdPoint<C>(C);

impl<C: Color> KdPoint for ColorKdPoint<C> {
    type Scalar = i64;
    type Dim = typenum::U4;

    fn at(&self, i: usize) -> Self::Scalar {
        let raw_color = self.0.into();
        let components: [u8; 4] = raw_color.into();
        components[i] as i64
    }
}
