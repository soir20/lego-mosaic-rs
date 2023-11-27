mod ldraw;

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::hash::Hash;
use boolvec::BoolVec;
use image::{DynamicImage, GenericImageView, Pixel};
use palette::color_difference::Wcag21RelativeContrast;
use palette::Srgba;

// ====================
// PUBLIC TRAITS
// ====================

pub trait Color: Copy + Default + Eq + Hash + Into<RawColor> {}

pub trait Brick: Copy + Hash + Eq {
    fn length(&self) -> u8;

    fn width(&self) -> u8;

    fn height(&self) -> u8;

    fn unit_brick(&self) -> Self;

    fn rotate(&self) -> Self;
}

// ====================
// PUBLIC STRUCTS
// ====================

pub struct Mosaic<B, C> {
    chunks: Vec<Chunk<B, C>>
}

impl<B: Brick, C: Color> Mosaic<B, C> {

    pub fn from_image(image: &DynamicImage, palette: &[C], unit_brick: B) -> Self {
        assert_unit_brick(unit_brick);

        let raw_colors: Pixels<RawColor> = image.into();
        let colors = raw_colors.with_palette(palette);

        let area = colors.values_by_row.len();
        let x_size = colors.x_size;
        let y_size = area / x_size;

        let mut visited = BoolVec::filled_with(area, false);
        let mut queue = VecDeque::new();
        let mut chunks = Vec::new();

        for start_y in 0..y_size {
            for start_x in 0..x_size {
                if was_visited(&visited, start_x, start_y, x_size) {
                    continue;
                }

                let start_color = colors.value(start_x, start_y);
                queue.push_back((start_x, start_y));

                let mut coordinates = Vec::new();
                let mut min_x = start_x;
                let mut min_y = start_y;
                let mut max_x = start_x;

                while !queue.is_empty() {
                    let (x, y) = queue.pop_front().unwrap();

                    if was_visited(&visited, x, y, x_size) {
                        continue;
                    }
                    visited.set(y * x_size + x, true);

                    coordinates.push((x, y));

                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);

                    if x > 0 && is_new_pos::<C>(&visited, &colors, x - 1, y, x_size, start_color) {
                        queue.push_back((x - 1, y));
                    }

                    if x < x_size - 1 && is_new_pos::<C>(&visited, &colors, x + 1, y, x_size, start_color) {
                        queue.push_back((x + 1, y));
                    }

                    if y > 0 && is_new_pos::<C>(&visited, &colors, x, y - 1, x_size, start_color) {
                        queue.push_back((x, y - 1));
                    }

                    if y < y_size - 1 && is_new_pos::<C>(&visited, &colors, x, y + 1, x_size, start_color) {
                        queue.push_back((x, y + 1));
                    }
                }

                let chunk_x_size = max_x - min_x + 1;
                let mut bricks = Vec::with_capacity(coordinates.len());
                let mut ys_included = vec![BTreeSet::new(); chunk_x_size];

                for (x, y) in coordinates {
                    let rel_x = x - min_x;
                    let rel_y = y - min_y;

                    bricks.push(PlacedBrick {
                        x: rel_x as u16,
                        y: rel_y as u16,
                        z: 0,
                        brick: unit_brick,
                        rotate: false
                    });

                    ys_included[rel_x].insert(rel_y as u16);
                }

                chunks.push(Chunk {
                    unit_brick,
                    color: start_color,
                    x: min_x as u16,
                    y: min_y as u16,
                    z: 0,
                    x_size: chunk_x_size as u16,
                    z_size: 1,
                    ys_included,
                    bricks,
                })
            }
        }

        Mosaic { chunks }
    }

    pub fn reduce_bricks(self, bricks: &[B]) -> Self {
        let bricks_by_z_size: HashMap<B, BTreeMap<u16, Vec<AreaSortedBrick<B>>>> = bricks.iter()
            .fold(HashMap::new(), |mut partitions, &brick| {
                let unit_brick = assert_unit_brick(brick.unit_brick());
                let entry = partitions.entry(unit_brick).or_insert_with(Vec::new);
                entry.push(brick);

                if brick.length() != brick.width() {
                    entry.push(brick.rotate());
                }

                partitions
            })
            .into_iter()
            .map(|(unit_brick, bricks)| (unit_brick, Mosaic::<B, C>::partition_by_z_size(bricks)))
            .collect();

        let chunks = self.chunks.into_iter()
            .map(|chunk| {
                let bricks_by_z_size = &bricks_by_z_size[&chunk.unit_brick];
                chunk.reduce_bricks(bricks_by_z_size)
            })
            .collect();

        Mosaic { chunks }
    }

    pub fn make_3d(self, height: u16, flip: bool) -> Self {
        let height_map = self.height_map(height, flip);
        Mosaic {
            chunks: self.chunks.into_iter()
                .map(|chunk| {
                    let key = chunk.color;
                    chunk.set_z_size(height_map[&key])
                })
                .collect()
        }
    }

    fn height_map(&self, z_size: u16, flip: bool) -> HeightMap<C> {
        if z_size == 0 {
            return HeightMap::new();
        }

        let (min_luma, max_luma) = self.chunks.iter()
            .map(|chunk| {
                let srgba_f32: Srgba<f32> = chunk.color.into().into_format();
                srgba_f32.relative_luminance().luma
            })
            .fold((1.0f32, 0.0f32), |(min, max), luma| (min.min(luma), max.max(luma)));

        let range = max_luma - min_luma;
        let max_layer_index = z_size - 1;

        let mut height_map = HeightMap::new();

        self.chunks.iter().for_each(|chunk| {
            let color = chunk.color;
            let entry = height_map.entry(color);
            entry.or_insert_with(|| {
                let srgba_f32: Srgba<f32> = color.into().into_format();
                let luma = srgba_f32.relative_luminance().luma;
                let mut range_rel_luma = (luma - min_luma) / range;
                range_rel_luma = if flip { 1.0 - range_rel_luma } else { range_rel_luma };

                /* z_size must be u16 because the max integer a 32-bit float can represent
                   exactly is 2^24 + 1 (more than u16::MAX but less than u32::MAX). */
                (range_rel_luma * max_layer_index as f32).round() as u16 + 1

            });
        });

        height_map
    }

    fn partition_by_z_size(bricks: Vec<B>) -> BTreeMap<u16, Vec<AreaSortedBrick<B>>> {
        bricks.into_iter().fold(BTreeMap::new(), |mut partitions, brick| {
            partitions.entry(brick.height()).or_insert_with(Vec::new).push(brick);
            partitions
        })
            .into_iter()
            .filter(|(_, bricks)| bricks.iter().any(|brick| brick.length() == 1 && brick.width() == 1))
            .map(|(z_size, bricks)| {
                let mut sizes = Vec::with_capacity(bricks.len());
                for brick in bricks {
                    sizes.push(AreaSortedBrick { brick, rotate: false });

                    if brick.length() != brick.width() {
                        sizes.push(AreaSortedBrick { brick, rotate: true });
                    }
                }

                sizes.sort();

                (z_size as u16, sizes)
            })
            .collect()
    }

}

// ====================
// PRIVATE TYPE ALIASES
// ====================

type RawColor = Srgba<u8>;

type HeightMap<C> = HashMap<C, u16>;

// ====================
// PRIVATE FUNCTIONS
// ====================

fn was_visited(visited: &BoolVec, x: usize, y: usize, x_size: usize) -> bool {
    visited.get(y * x_size + x).unwrap()
}

fn is_new_pos<C: Color>(visited: &BoolVec, colors: &Pixels<C>, x: usize, y: usize, x_size: usize, start_color: C) -> bool {
    !was_visited(visited, x, y, x_size) && colors.value(x, y) == start_color
}

fn assert_unit_brick<B: Brick>(brick: B) -> B {
    assert_eq!(1, brick.length());
    assert_eq!(1, brick.width());
    assert_eq!(1, brick.height());

    brick
}

// ====================
// PRIVATE STRUCTS
// ====================

struct AreaSortedBrick<B> {
    brick: B,
    rotate: bool
}

impl<B: Brick> AreaSortedBrick<B> {
    fn x_size(&self) -> u8 {
        match self.rotate {
            true => self.brick.width(),
            false => self.brick.length()
        }
    }

    fn y_size(&self) -> u8 {
        match self.rotate {
            true => self.brick.length(),
            false => self.brick.width()
        }
    }

    fn area(&self) -> u16 {
        self.x_size() as u16 * self.y_size() as u16
    }
}

impl<B: Brick> Eq for AreaSortedBrick<B> {}

impl<B: Brick> PartialEq<Self> for AreaSortedBrick<B> {
    fn eq(&self, other: &Self) -> bool {
        self.brick == other.brick
    }
}

impl<B: Brick> PartialOrd<Self> for AreaSortedBrick<B> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<B: Brick> Ord for AreaSortedBrick<B> {
    fn cmp(&self, other: &Self) -> Ordering {
        let area1 = self.area();
        let area2 = other.area();

        // Sort in descending order
        area2.cmp(&area1)

    }
}

struct LayerPlacedBrick<B> {
    x: u16,
    y: u16,
    brick: B,
    rotate: bool
}

struct PlacedBrick<B> {
    x: u16,
    y: u16,
    z: u16,
    brick: B,
    rotate: bool
}

struct Chunk<B, C> {
    unit_brick: B,
    color: C,
    x: u16,
    y: u16,
    z: u16,
    x_size: u16,
    z_size: u16,
    ys_included: Vec<BTreeSet<u16>>,
    bricks: Vec<PlacedBrick<B>>
}

impl<B: Brick, C: Color> Chunk<B, C> {

    pub fn set_z_size(mut self, new_z_size: u16) -> Self {
        let new_layers = new_z_size.abs_diff(self.z_size);

        if self.z_size > new_z_size {
            let new_min_z = new_layers;
            self.bricks = self.bricks.into_iter()
                .flat_map(|brick| {
                    if brick.z >= new_min_z {
                        return vec![PlacedBrick {
                            x: brick.x,
                            y: brick.y,
                            z: brick.z - new_min_z,
                            brick: brick.brick,
                            rotate: brick.rotate,
                        }];
                    }

                    let brick_z_above = brick.z + brick.brick.height() as u16;
                    if brick_z_above <= new_min_z {
                        return vec![];
                    }

                    let zs_to_replace = brick_z_above - new_min_z;
                    let mut new_bricks = Vec::with_capacity(
                        zs_to_replace as usize * brick.brick.length() as usize * brick.brick.width() as usize
                    );

                    for z in 0..(brick_z_above - new_min_z) {
                        for x in brick.x..(brick.x + brick.brick.length() as u16) {
                            for y in brick.y..(brick.y + brick.brick.width() as u16) {
                                new_bricks.push(PlacedBrick {
                                    x,
                                    y,
                                    z,
                                    brick: self.unit_brick,
                                    rotate: false
                                });
                            }
                        }
                    }

                    new_bricks
                })
                .collect();
        } else {
            for z in self.z_size..(self.z_size + new_layers) {
                for x in 0..self.x_size {
                    for &y in self.ys_included[x as usize].iter() {
                        self.bricks.push(PlacedBrick {
                            x,
                            y,
                            z,
                            brick: self.unit_brick,
                            rotate: false
                        });
                    }
                }
            }
        }

        self.z_size = new_z_size;

        self
    }

    pub fn reduce_bricks(self, bricks_by_z_size: &BTreeMap<u16, Vec<AreaSortedBrick<B>>>) -> Self {
        let mut last_z_index = 0;
        let mut remaining_height = self.z_size;
        let mut layers = Vec::new();

        for &z_size in bricks_by_z_size.keys().rev() {
            let layers_of_size = remaining_height / z_size;
            remaining_height %= z_size;

            for _ in 0..layers_of_size {
                layers.push((z_size, last_z_index));
                last_z_index += z_size;
            }
        }

        let bricks: Vec<PlacedBrick<B>> = layers.into_iter().flat_map(|(z_size, z_index)| {
            let sizes = &bricks_by_z_size[&z_size];
            Chunk::<B, C>::reduce_single_layer(sizes, self.x_size, self.ys_included.clone())
                .into_iter()
                .map(move |placed_brick| PlacedBrick {
                    x: self.x + placed_brick.x,
                    y: self.y + placed_brick.y,
                    z: z_index,
                    brick: placed_brick.brick,
                    rotate: placed_brick.rotate
                })
        }).collect();

        Chunk {
            unit_brick: self.unit_brick,
            color: self.color,
            x: self.x,
            y: self.y,
            z: self.z,
            x_size: self.x_size,
            z_size: self.z_size,
            ys_included: self.ys_included,
            bricks,
        }
    }

    fn reduce_single_layer(sizes: &[AreaSortedBrick<B>], x_size: u16, mut ys_included_by_x: Vec<BTreeSet<u16>>) -> Vec<LayerPlacedBrick<B>> {
        let mut bricks = Vec::new();

        for x in 0..x_size {
            let x_index = x as usize;

            while !ys_included_by_x[x_index].is_empty() {
                let ys_included = &ys_included_by_x[x_index];

                if let Some(&y) = ys_included.first() {
                    for size in sizes {
                        if Chunk::<B, C>::fits(x, y, size.x_size(), size.y_size(), &ys_included_by_x) {
                            Chunk::<B, C>::remove_brick(x, y, size.x_size(), size.y_size(), &mut ys_included_by_x);
                            bricks.push(LayerPlacedBrick {
                                brick: size.brick,
                                x,
                                y,
                                rotate: size.rotate
                            })
                        }
                    }
                }
            }
        }

        bricks
    }

    fn fits(x: u16, y: u16, x_size: u8, y_size: u8, ys_included_by_x: &[BTreeSet<u16>]) -> bool {
        let max_x = x + x_size as u16;
        let max_y = y + y_size as u16;

        if max_x as usize > ys_included_by_x.len() {
            return false;
        }

        for test_x in x..max_x {
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

        for ys_included in ys_included_by_x.iter_mut().take(max_x).skip(min_x) {
            for cur_y in y..max_y {
                ys_included.remove(&cur_y);
            }
        }
    }
}

struct Pixels<T> {
    values_by_row: Vec<T>,
    x_size: usize
}

impl<T: Copy> Pixels<T> {
    pub fn value(&self, x: usize, y: usize) -> T {
        self.values_by_row[y * self.x_size + x]
    }
}

impl From<&DynamicImage> for Pixels<RawColor> {
    fn from(image: &DynamicImage) -> Self {
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

impl Pixels<RawColor> {
    pub fn with_palette<C: Color>(self, palette: &[C]) -> Pixels<C> {
        let new_colors = self.values_by_row.into_iter()
            .map(|color| Self::find_similar_color(color, palette))
            .collect();
        Pixels { values_by_row: new_colors, x_size: self.x_size }
    }

    fn find_similar_color<C: Color>(color: RawColor, palette: &[C]) -> C {
        let mut best_distance = u32::MAX;
        let mut best_color = C::default();

        for &palette_color in palette {
            let distance = Self::distance_squared(color, palette_color.into());

            if distance < best_distance {
                best_distance = distance;
                best_color = palette_color;
            }
        }

        best_color
    }

    fn distance_squared(color1: RawColor, color2: RawColor) -> u32 {

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
