use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::hash::Hash;
use boolvec::BoolVec;
use image::{DynamicImage, GenericImageView, Pixel};
use palette::color_difference::Wcag21RelativeContrast;
use palette::Srgba;

pub type Color = Srgba<u8>;

pub trait Brick: Copy + Hash + Eq {
    fn x_size(&self) -> u8;

    fn y_size(&self) -> u8;

    fn z_size(&self) -> u8;

    fn unit_brick(&self) -> Self;
}

struct AreaSortedBrick<B> {
    brick: B
}

impl<B: Brick> AreaSortedBrick<B> {
    fn x_size(&self) -> u8 {
        self.brick.x_size()
    }

    fn y_size(&self) -> u8 {
        self.brick.y_size()
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
    brick: B
}

struct PlacedBrick<B> {
    x: u16,
    y: u16,
    z: u16,
    brick: B
}

struct Chunk<B> {
    unit_brick: B,
    color: Color,
    x: u16,
    y: u16,
    z: u16,
    x_size: u16,
    z_size: u16,
    ys_included: Vec<BTreeSet<u16>>,
    bricks: Vec<PlacedBrick<B>>
}

impl<B: Brick> Chunk<B> {

    fn raise(mut self, new_z_size: u16) -> Self {
        assert!(self.z_size <= new_z_size);
        let new_layers = new_z_size - self.z_size;

        for x in 0..self.x_size {
            for &y in self.ys_included[x as usize].iter() {
                for z in 0..new_layers {
                    self.bricks.push(PlacedBrick {
                        x,
                        y,
                        z,
                        brick: self.unit_brick,
                    });
                }
            }

        }

        self
    }

    fn reduce_bricks(self, bricks_by_z_size: &BTreeMap<u16, Vec<AreaSortedBrick<B>>>) -> Self {
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
            Chunk::<B>::reduce_single_layer(sizes, self.x_size, self.ys_included.clone())
                .into_iter()
                .map(move |placed_brick| PlacedBrick {
                    x: self.x + placed_brick.x,
                    y: self.y + placed_brick.y,
                    z: z_index,
                    brick: placed_brick.brick,
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
                        if Chunk::<B>::fits(x, y, size.x_size(), size.y_size(), &ys_included_by_x) {
                            Chunk::<B>::remove_brick(x, y, size.x_size(), size.y_size(), &mut ys_included_by_x);
                            bricks.push(LayerPlacedBrick {
                                brick: size.brick,
                                x,
                                y,
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

        for cur_x in min_x..max_x {
            let ys_included = &mut ys_included_by_x[cur_x];

            for cur_y in y..max_y {
                ys_included.remove(&cur_y);
            }
        }
    }
}

pub struct Mosaic<B> {
    chunks: Vec<Chunk<B>>
}

impl<B: Brick> Mosaic<B> {

    pub fn from_image(image: &DynamicImage, palette: &[Color], unit_brick: B) -> Self {
        assert_unit_brick(unit_brick);

        let raw_colors: Pixels<Srgba<u8>> = image.into();
        let colors = raw_colors.with_palette(palette);

        let area = colors.values_by_row.len();
        let x_size = colors.x_size;
        let y_size = area / x_size;

        let mut visited = BoolVec::filled_with(area, false);
        let mut queue = VecDeque::new();
        let mut chunks = Vec::new();

        for start_y in 0..y_size {
            for start_x in 0..x_size {
                if was_visited(&mut visited, start_x, start_y, x_size) {
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

                    if was_visited(&mut visited, x, y, x_size) {
                        continue;
                    }
                    visited.set(y * x_size + x, true);

                    coordinates.push((x, y));

                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);

                    if x > 0 && is_new_pos::<B>(&visited, &colors, x - 1, y, x_size, start_color) {
                        queue.push_back((x - 1, y));
                    }

                    if x < x_size - 1 && is_new_pos::<B>(&visited, &colors, x + 1, y, x_size, start_color) {
                        queue.push_back((x + 1, y));
                    }

                    if y > 0 && is_new_pos::<B>(&visited, &colors, x, y - 1, x_size, start_color) {
                        queue.push_back((x, y - 1));
                    }

                    if y < y_size - 1 && is_new_pos::<B>(&visited, &colors, x, y + 1, x_size, start_color) {
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

    pub fn reduce_bricks(self, bricks: &[B]) -> Mosaic<B> {
        let bricks_by_z_size: HashMap<B, BTreeMap<u16, Vec<AreaSortedBrick<B>>>> = bricks.iter()
            .fold(HashMap::new(), |mut partitions, brick| {
                let unit_brick = assert_unit_brick(brick.unit_brick());
                partitions.entry(unit_brick).or_insert_with(|| Vec::new()).push(brick);
                partitions
            })
            .into_iter()
            .map(|(unit_brick, bricks)| (unit_brick, Mosaic::<B>::partition_by_z_size(bricks)))
            .collect();

        let chunks = self.chunks.into_iter()
            .map(|chunk| {
                let bricks_by_z_size = &bricks_by_z_size[&chunk.unit_brick];
                chunk.reduce_bricks(bricks_by_z_size)
            })
            .collect();

        Mosaic { chunks }
    }

    fn partition_by_z_size(bricks: Vec<&B>) -> BTreeMap<u16, Vec<AreaSortedBrick<B>>> {
        bricks.into_iter().fold(BTreeMap::new(), |mut partitions, brick| {
            partitions.entry(brick.z_size()).or_insert_with(|| Vec::new()).push(brick);
            partitions
        })
            .into_iter()
            .filter(|(_, bricks)| bricks.iter().any(|brick| brick.x_size() == 1 && brick.y_size() == 1))
            .map(|(z_size, bricks)| {
                let mut sizes: Vec<AreaSortedBrick<B>> = bricks.into_iter()
                    .map(|&brick| AreaSortedBrick { brick })
                    .collect();
                sizes.sort();

                (z_size as u16, sizes)
            })
            .collect()
    }

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

impl From<&DynamicImage> for Pixels<Srgba<u8>> {
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
}

fn assert_unit_brick<B: Brick>(brick: B) -> B {
    assert_eq!(1, brick.x_size());
    assert_eq!(1, brick.y_size());
    assert_eq!(1, brick.z_size());

    brick
}

#[cfg(test)]
mod tests {
    use std::hash::Hasher;
    use image::imageops::FilterType;
    use super::*;

    struct TestBrick {
        name: &'static str,
        x_size: u8,
        y_size: u8,
        z_size: u8,
    }

    impl Copy for TestBrick {}

    impl Clone for TestBrick {
        fn clone(&self) -> Self {
            TestBrick {
                name: self.name.clone(),
                x_size: self.x_size,
                y_size: self.y_size,
                z_size: self.z_size,
            }
        }
    }

    impl Hash for TestBrick {
        fn hash<H: Hasher>(&self, state: &mut H) {
            self.name.hash(state)
        }
    }

    impl Eq for TestBrick {}

    impl PartialEq<Self> for TestBrick {
        fn eq(&self, other: &Self) -> bool {
            self.name == other.name
        }
    }

    impl Brick for TestBrick {
        fn x_size(&self) -> u8 {
            self.x_size
        }

        fn y_size(&self) -> u8 {
            self.y_size
        }

        fn z_size(&self) -> u8 {
            self.z_size
        }

        fn unit_brick(&self) -> Self {
            TestBrick {
                name: "1x1x1",
                x_size: 1,
                y_size: 1,
                z_size: 1,
            }
        }
    }

    #[test]
    fn test() {
        let img = image::open("dragon.webp").unwrap();
        let resized = img.resize_exact(20, 20, FilterType::CatmullRom);

        let palette = vec![
            Color::new(255, 0, 0, 255),
            Color::new(0, 255, 0, 255),
            Color::new(0, 0, 255, 255),
        ];

        let brick2x2x1 = TestBrick {
            name: "2x2x1",
            x_size: 2,
            y_size: 2,
            z_size: 1,
        };
        let dup_brick = TestBrick {
            name: "dup",
            x_size: 2,
            y_size: 2,
            z_size: 1,
        };
        let brick1x1x1 = brick2x2x1.unit_brick();

        let mut mosaic = Mosaic::<TestBrick>::from_image(&resized, &palette, brick1x1x1);
        mosaic = mosaic.reduce_bricks(&vec![brick1x1x1, brick2x2x1, dup_brick]);

        for chunk in mosaic.chunks {
            println!("CHUNK {:?}", chunk.color);
            for brick in chunk.bricks {
                println!("x={}, y={}, z={}, x_size={}, y_size={}, z_size={}", brick.x, brick.y, brick.z, brick.brick.x_size, brick.brick.y_size, brick.brick.z_size);
            }
        }

        resized.save("dragon-small.png").unwrap();
    }
}
