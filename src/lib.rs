use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet, VecDeque};
use std::iter;
use boolvec::BoolVec;
use image::{DynamicImage, GenericImageView, Pixel};
use palette::color_difference::Wcag21RelativeContrast;
use palette::Srgba;

pub type Color = Srgba<u8>;

pub trait Brick {
    type UnitBrick;

    fn x_size(&self) -> u8;

    fn y_size(&self) -> u8;

    fn z_size(&self) -> u8;
}

pub trait UnitBrick {
    fn get() -> Self;
}

impl<B: UnitBrick> Brick for B {
    type UnitBrick = Self;

    fn x_size(&self) -> u8 {
        1
    }

    fn y_size(&self) -> u8 {
        1
    }

    fn z_size(&self) -> u8 {
        1
    }
}

struct BrickRow<B> {
    brick: B,
    len: u16,
    x: u16,
    y: u16,
    z: u16
}

#[derive(Clone)]
struct Dimension {
    x_size: u8,
    y_size: u8
}

impl Dimension {
    fn area(&self) -> u16 {
        self.x_size as u16 * self.y_size as u16
    }
}

impl Eq for Dimension {}

impl PartialEq<Self> for Dimension {
    fn eq(&self, other: &Self) -> bool {
        self.x_size == other.x_size && self.y_size == other.y_size
    }
}

impl PartialOrd<Self> for Dimension {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Dimension {
    fn cmp(&self, other: &Self) -> Ordering {
        let area1 = self.area();
        let area2 = other.area();

        // Sort in descending order
        area2.cmp(&area1)

    }
}

struct BrickIndex {
    index: usize,
    x: u16,
    y: u16,
    x_size: u8,
    y_size: u8
}

struct Chunk<U, B> {
    brick_type: U,
    color: Color,
    x: u16,
    y: u16,
    z: u16,
    x_size: u16,
    y_size: u16,
    z_size: u16,
    ys_included: Vec<BTreeSet<u16>>,
    bricks: Vec<(u16, u16, u16, B)>
}

impl<U, B: Brick<UnitBrick=U>> Chunk<U, B> {
    fn reduce_bricks(&self, bricks: &[B]) {
        // partition by type, 1x1 bricks
        // reduce chunks matching type
        // don't error if chunks can't be transformed--throw error when single-brick mosaic created
    }

    fn reduce_single_layer(sizes: &[Dimension], x_size: u16, ys_included_by_x: &mut [BTreeSet<u16>]) -> Vec<BrickIndex> {
        let mut bricks = Vec::new();

        for x in 0..x_size {
            let x_index = x as usize;

            while !ys_included_by_x[x_index].is_empty() {
                let ys_included = &ys_included_by_x[x_index];

                if let Some(&y) = ys_included.first() {
                    for brick_index in 0..sizes.len() {
                        let brick = &sizes[brick_index];

                        // need to check that at least one fits
                        if Chunk::<U, B>::fits(x, y, brick.x_size, brick.y_size, ys_included_by_x) {
                            Chunk::<U, B>::remove_brick(x, y, brick.x_size, brick.y_size, ys_included_by_x);
                            bricks.push(BrickIndex {
                                index: brick_index,
                                x,
                                y,
                                x_size: brick.x_size,
                                y_size: brick.y_size
                            })
                        }
                    }
                }
            }
        }

        bricks
    }

    fn fits(x: u16, y: u16, x_size: u8, y_size: u8, ys_included_by_x: &[BTreeSet<u16>]) -> bool {
        let max_y = y + y_size as u16;

        for test_x in x..(x + x_size as u16) {
            if ys_included_by_x[test_x as usize].range(y..max_y).count() < y_size as usize {
                return false;
            }
        }

        true
    }

    fn remove_brick(x: u16, y: u16, x_size: u8, y_size: u8, ys_included_by_x: &mut [BTreeSet<u16>]) {
        let min_x = x as usize;
        let max_x = x as usize + x_size as usize;
        let max_y = y + y_size as u16;

        for cur_x in min_x..max_x {
            let mut ys_included = &mut ys_included_by_x[cur_x];

            for cur_y in y..max_y {
                ys_included.remove(&cur_y);
            }
        }
    }
}

/*pub struct Mosaic<P> {
    chunks: Vec<Chunk<P>>
}

impl<P: Copy> Mosaic<P> {

    pub fn from_image(image: DynamicImage, palette: &[Color], brick: P) -> Self {
        let raw_colors: Pixels<Srgba<u8>> = image.into();
        let colors = raw_colors.with_palette(palette);

        let area = colors.values_by_row.len();
        let x_size = colors.x_size;
        let y_size = area / x_size;

        let mut visited = BoolVec::with_capacity(area);
        let mut queue = VecDeque::new();
        let mut chunk_pos = HashSet::new();

        let mut chunks = Vec::new();

        for start_y in 0..y_size {
            for start_x in 0..x_size {
                if was_visited(&mut visited, start_x, start_y, x_size) {
                    continue;
                }

                let start_color = colors.value(start_x, start_y);
                queue.push_back((start_x, start_y));

                let mut min_x = start_x;
                let mut min_y = start_y;
                let mut max_x = start_x;
                let mut max_y = start_y;

                while !queue.is_empty() {
                    let (x, y) = queue.pop_front().unwrap();
                    visited.set(y * x_size + x, true);
                    chunk_pos.insert((x, y));
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = min_y.max(y);

                    if x > 0 && is_new_pos::<P>(&visited, &colors, x - 1, y, x_size, start_color) {
                        queue.push_back((x - 1, y));
                    }

                    if x < x_size - 1 && is_new_pos::<P>(&visited, &colors, x + 1, y, x_size, start_color) {
                        queue.push_back((x + 1, y));
                    }

                    if y > 0 && is_new_pos::<P>(&visited, &colors, x, y - 1, x_size, start_color) {
                        queue.push_back((x, y - 1));
                    }

                    if y < y_size - 1 && is_new_pos::<P>(&visited, &colors, x, y + 1, x_size, start_color) {
                        queue.push_back((x, y + 1));
                    }
                }

                let chunk_x_size = max_x - min_x + 1;
                let chunk_y_size = max_y - min_y + 1;

                let mut excluded_xs = vec![BTreeSet::new(); chunk_y_size];
                let mut excluded_ys = vec![BTreeSet::new(); chunk_x_size];

                for y in min_y..=max_y {
                    for x in min_x..=max_x {
                        if !chunk_pos.contains(&(x, y)) {
                            excluded_xs[y].insert(x as u32);
                            excluded_ys[x].insert(y as u32);
                        }
                    }
                }

                chunks.push(Chunk {
                    brick,
                    color: start_color,
                    x: min_x as u32,
                    y: min_y as u32,
                    x_size: chunk_x_size as u32,
                    y_size: chunk_y_size as u32,
                    z_size: 1,
                    excluded_xs,
                    excluded_ys
                });
                chunk_pos.clear();
            }
        }

        Mosaic { chunks }
    }

    //pub fn reduce_bricks(self) -> Self {

    //}

}

fn was_visited(visited: &BoolVec, x: usize, y: usize, x_size: usize) -> bool {
    visited.get(y * x_size + x).unwrap()
}

fn is_new_pos<P: Copy>(visited: &BoolVec, colors: &Pixels<Color>, x: usize, y: usize, x_size: usize, start_color: Color) -> bool {
    !was_visited(&visited, x, y, x_size) && colors.value(x, y) == start_color
}

struct Pixels<T> {
    values_by_row: Vec<T>,
    x_size: usize
}

impl<T: Copy> Pixels<T> {
    fn value(&self, x: usize, y: usize) -> T {
        self.values_by_row[y * self.x_size + x]
    }
}

impl From<DynamicImage> for Pixels<Srgba<u8>> {
    fn from(image: DynamicImage) -> Self {
        let x_size = image.width() as usize;
        let y_size = image.height() as usize;
        let mut colors = Vec::with_capacity(x_size * y_size);

        for y in 0..y_size {
            for x in 0..x_size {
                let color = image.get_pixel(x as u32, y as u32).to_rgba();
                let channels = color.channels();
                let red = channels[0];
                let green = channels[1];
                let blue = channels[2];
                let alpha = channels[3];

                colors.push(Srgba::new(red, green, blue, alpha));
            }
        }

        Pixels { values_by_row: colors, x_size }
    }
}

impl Pixels<Srgba<u8>> {
    pub fn with_palette(self, palette: &[Color]) -> Pixels<Color> {
        let new_colors = self.values_by_row.into_iter()
            .map(|color| Self::find_similar_color(color, palette))
            .collect();
        Pixels { values_by_row: new_colors, x_size: self.x_size }
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

type HeightMap = HashMap<u64, u16>;
impl Pixels<Color> {
    pub fn height_map(&self, layers: u16, flip: bool) -> HeightMap {
        if layers == 0 {
            return HeightMap::new();
        }

        let (min_luma, max_luma) = self.values_by_row.iter()
            .map(|color| {
                let srgba_f32: Srgba<f32> = color.into_format();
                srgba_f32.relative_luminance().luma
            })
            .fold((0.0f32, 1.0f32), |(min, max), luma| (min.min(luma), max.max(luma)));

        let range = max_luma - min_luma;
        let max_layer_index = layers - 1;

        let mut height_map = HeightMap::new();

        self.values_by_row.iter().for_each(|color| {
            let entry = height_map.entry(color_as_key(*color));
            entry.or_insert_with(|| {
                let srgba_f32: Srgba<f32> = color.into_format();
                let luma = srgba_f32.relative_luminance().luma;
                let mut range_rel_luma = (luma - min_luma) / range;
                range_rel_luma = if flip { 1.0 - range_rel_luma } else { range_rel_luma };

                /* Layers must be u16 because the max integer a 32-bit float can represent
                   exactly is 2^24 + 1 (more than u16::MAX but less than u32::MAX). */
                (range_rel_luma * max_layer_index as f32).round() as u16 + 1

            });
        });

        height_map
    }
}

fn color_as_key(color: Color) -> u64 {
    let mut key = 0u64;
    key |= (color.red as u64) << 48;
    key |= (color.green as u64) << 32;
    key |= (color.blue as u64) << 16;
    key |= color.alpha as u64;
    key
}*/
