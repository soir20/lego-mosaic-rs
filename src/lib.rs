mod ldraw;

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::hash::Hash;
use boolvec::BoolVec;
use image::{DynamicImage, GenericImageView, Pixel};
use palette::Srgba;

// This API uses l, w, and h coordinate axes, which refer to length, width, and height,
// respectively. A brick's length refers to its size along the l axis, a brick's
// width refers to its size along the w axis, and a brick's height refers to its size
// along the h axis.
//
// From a bird's eye view, increasing l refers to moving east, while increasing w
// refers to moving south. Increasing h refers to increasing altitude above the plane.
// This is consistent with image editors, as well as the image crate, which put the
// origin at the top left.
//
// The x, y, and z axes are not used because l, w, and h more clearly map to brick size,
// and many existing programs have conflicting definitions of what y and z refer to.
// For example, in the LDraw format, decreasing y is analogous to increasing altitude,
// and x and z are horizontal axes. In other programs, z is the vertical axis instead
// of y, and increasing z is analogous to increasing altitude. With the l, w, and h axes,
// the user must explicitly consider the mapping between this API's axes and another
// program's x, y, and z axes, rather than assuming the API's axes conform to those of
// another program.

// ====================
// PUBLIC TRAITS
// ====================

pub trait Color: Copy + Default + Eq + Hash + Into<RawColor> {}

pub trait Brick: Copy + Hash + Eq {
    fn length(&self) -> u8;

    fn width(&self) -> u8;

    fn height(&self) -> u8;

    fn unit_brick(&self) -> Self;

    fn rotate_90(&self) -> Self;
}

// ====================
// PUBLIC STRUCTS
// ====================

#[derive(Clone)]
pub struct PlacedBrick<B> {
    l: u16,
    w: u16,
    h: u16,
    brick: B
}

impl<B> PlacedBrick<B> {
    pub fn l(&self) -> u16 {
        self.l
    }

    pub fn w(&self) -> u16 {
        self.w
    }

    pub fn h(&self) -> u16 {
        self.h
    }
}

pub struct PlacedColor<C> {
    l: u16,
    w: u16,
    color: C
}

impl<C: Color> PlacedColor<C> {
    pub fn l(&self) -> u16 {
        self.l
    }

    pub fn w(&self) -> u16 {
        self.w
    }

    pub fn color(&self) -> C {
        self.color
    }
}

pub struct Mosaic<B, C> {
    chunks: Vec<Chunk<B, C>>
}

impl<B: Brick, C: Color> Mosaic<B, C> {
    pub fn from_image(image: &DynamicImage,
                      palette: &[C],
                      mut height_fn: impl FnMut(u16, u16, C) -> u16,
                      mut brick_fn: impl FnMut(u16, u16, u16, C) -> B) -> Self {

        // Cache colors, heights, and bricks so functions are only called once per point
        let raw_colors: Pixels<RawColor> = image.into();
        let colors = raw_colors.with_palette(palette);
        let length = colors.length;
        let width = colors.values_by_row.len() / colors.length.max(1);

        let height_map = HeightMap::from_fn(
            |l, w| height_fn(l as u16, w as u16, colors.value(l, w)),
            length,
            width
        );
        let max_height = height_map.max().map_or(0, |max| *max);

        let mut brick_cache = BTreeMap::new();

        // Build contiguous 3D chunks (with same color and brick) of the mosaic
        let chunks = Mosaic::<B, C>::build_chunks(
            length as u16,
            width as u16,
            max_height,
            |l, w| height_map.value(l as usize, w as usize),
            |l, w, h, color| *brick_cache.entry((l, w, h)).or_insert_with(|| brick_fn(l, w, h, color)),
            |l, w| colors.value(l as usize, w as usize)
        );

        Mosaic::new(chunks)
    }

    pub fn reduce_bricks(self, bricks: &[B]) -> Self {
        let bricks_by_height: HashMap<B, BTreeMap<u16, Vec<AreaSortedBrick<B>>>> = bricks.iter()
            .fold(HashMap::new(), |mut partitions, &brick| {

                // Consider each brick's associated unit brick as its type
                let unit_brick = assert_unit_brick(brick.unit_brick());
                let entry = partitions.entry(unit_brick).or_insert_with(Vec::new);
                entry.push(brick);

                // A square brick rotated 90 degrees is redundant
                if brick.length() != brick.width() {
                    entry.push(brick.rotate_90());
                }

                partitions
            })
            .into_iter()
            .map(|(unit_brick, bricks)| (unit_brick, Mosaic::<B, C>::partition_by_height(bricks)))
            .collect();

        let chunks = self.chunks.into_iter()
            .map(|chunk| {
                let bricks_by_height = &bricks_by_height[&chunk.unit_brick];
                chunk.reduce_bricks(bricks_by_height)
            })
            .collect();

        Mosaic::new(chunks)
    }

    fn new(chunks: Vec<Chunk<B, C>>) -> Self {
        Mosaic {
            chunks: chunks.into_iter()
                .filter(|chunk| chunk.length > 0 && chunk.width > 0 && chunk.height > 0)
                .collect()
        }
    }

    fn partition_by_height(bricks: Vec<B>) -> BTreeMap<u16, Vec<AreaSortedBrick<B>>> {
        /* Ensure that every h size has at least one 1x1 brick so that we are certain we can fill
           a layer of that h size. */
        bricks.into_iter().fold(BTreeMap::new(), |mut partitions, brick| {
            partitions.entry(brick.height()).or_insert_with(Vec::new).push(brick);
            partitions
        })
            .into_iter()
            .filter(|(_, bricks)| bricks.iter().any(|brick| brick.length() == 1 && brick.width() == 1))
            .map(|(height, bricks)| {
                /* Sort bricks by area so that larger bricks are chosen first. We don't need to
                   sort by volume because the brick-filling algorithm only needs to consider 2D
                   space. */
                let mut sizes: Vec<_> = bricks.into_iter()
                    .map(|brick| AreaSortedBrick { brick })
                    .collect();
                sizes.sort();

                (height as u16, sizes)
            })
            .collect()
    }

    fn build_chunks(length: u16,
                    width: u16,
                    max_height: u16,
                    mut height_fn: impl FnMut(u16, u16) -> u16,
                    mut brick_fn: impl FnMut(u16, u16, u16, C) -> B,
                    color_fn: impl Fn(u16, u16) -> C) -> Vec<Chunk<B, C>> {
        let mut visited = BoolVec::filled_with(length as usize * width as usize * max_height as usize, false);
        let mut coords_to_visit = VecDeque::new();
        let mut chunks = Vec::new();

        /* An iterative breadth-first search that explores contiguous chunks of the mosaic with
           the same brick type and color, similar to the classic island-finding problem */
        for start_w in 0..width {
            for start_l in 0..length {
                let start_height = height_fn(start_l, start_w);

                for start_h in 0..start_height {
                    if was_visited(&visited, start_l, start_w, start_h, length, width) {
                        continue;
                    }

                    let start_color = color_fn(start_l, start_w);
                    let start_brick = assert_unit_brick(brick_fn(start_l, start_w, start_h, start_color));
                    coords_to_visit.push_back((start_l, start_w, start_h));

                    let mut coords_in_chunk = BTreeMap::new();
                    let mut min_l = start_l;
                    let mut min_w = start_w;
                    let mut max_l = start_l;
                    let mut max_w = start_w;
                    let mut min_h = start_h;

                    while !coords_to_visit.is_empty() {
                        let (l, w, h) = coords_to_visit.pop_front().unwrap();
                        let height = height_fn(l, w);

                        // Avoid an infinite loop by visiting no point twice
                        if was_visited(&visited, l, w, h, length, width) {
                            continue;
                        }
                        visited.set(visited_index(l, w, h, length, width), true);

                        coords_in_chunk.entry(h).or_insert_with(BTreeSet::new).insert((l, w));

                        min_l = min_l.min(l);
                        min_w = min_w.min(w);
                        max_l = max_l.max(l);
                        max_w = max_w.max(w);
                        min_h = min_h.min(h);

                        // Add position to the west to explore later
                        if l > 0 && height_fn(l - 1, w) > h
                            && is_new_pos::<B, C>(&visited, &mut brick_fn, &color_fn, l - 1, w, h, length, width, start_brick, start_color) {
                            coords_to_visit.push_back((l - 1, w, h));
                        }

                        // Add position to the east to explore later
                        if l < length - 1 && height_fn(l + 1, w) > h
                            && is_new_pos::<B, C>(&visited, &mut brick_fn, &color_fn, l + 1, w, h, length, width, start_brick, start_color) {
                            coords_to_visit.push_back((l + 1, w, h));
                        }

                        // Add position to the south to explore later
                        if w > 0 && height_fn(l, w - 1) > h
                            && is_new_pos::<B, C>(&visited, &mut brick_fn, &color_fn, l, w - 1, h, length, width, start_brick, start_color) {
                            coords_to_visit.push_back((l, w - 1, h));
                        }

                        // Add position to the north to explore later
                        if w < width - 1 && height_fn(l, w + 1) > h
                            && is_new_pos::<B, C>(&visited, &mut brick_fn, &color_fn, l, w + 1, h, length, width, start_brick, start_color) {
                            coords_to_visit.push_back((l, w + 1, h));
                        }

                        // Add position below to explore later
                        if h > 0 && is_new_pos::<B, C>(&visited, &mut brick_fn, &color_fn, l, w, h - 1, length, width, start_brick, start_color) {
                            coords_to_visit.push_back((l, w, h - 1));
                        }

                        // Add position above to explore later
                        if h < height - 1 && is_new_pos::<B, C>(&visited, &mut brick_fn, &color_fn, l, w, h + 1, length, width, start_brick, start_color) {
                            coords_to_visit.push_back((l, w, h + 1));
                        }
                    }

                    let mut slices = Mosaic::<B, C>::slice_chunk(
                        coords_in_chunk,
                        start_brick,
                        start_color,
                        min_l,
                        max_l,
                        min_w,
                        max_w,
                        min_h
                    );
                    chunks.append(&mut slices);
                }
            }
        }

        chunks
    }

    fn slice_chunk(coords_in_chunk: BTreeMap<u16, BTreeSet<(u16, u16)>>,
                   unit_brick: B, color: C, min_l: u16, max_l: u16,
                   min_w: u16, max_w: u16, min_h: u16) -> Vec<Chunk<B, C>> {
        if coords_in_chunk.is_empty() {
            return Vec::new();
        }

        let mut heights = Vec::new();
        let mut height: u16 = 0;
        let mut last_coords = coords_in_chunk.values().next().unwrap();
        let mut iter = coords_in_chunk.iter();

        loop {
            if let Some((_, coords)) = iter.next() {
                if last_coords != coords {
                    heights.push(height);
                    height = 0;
                }

                last_coords = coords;
                height += 1;
            } else {
                if height > 0 {
                    heights.push(height);
                }
                break
            }
        }

        // Compute relative coordinates for every point inside the fully-explored chunk
        let chunk_length = max_l - min_l + 1;
        let chunk_width = max_w - min_w + 1;

        let mut slices = Vec::new();
        let mut slice_h = min_h;

        for height in heights {
            let coords_in_slice = &coords_in_chunk[&slice_h];
            let mut bricks = Vec::with_capacity(coords_in_slice.len());
            let mut ws_included = vec![BTreeSet::new(); chunk_length as usize];

            for &(l, w) in coords_in_slice {
                let rel_l = l - min_l;
                let rel_w = w - min_w;
                ws_included[rel_l as usize].insert(rel_w);

                for rel_h in 0..height {
                    bricks.push(PlacedBrick {
                        l: rel_l,
                        w: rel_w,
                        h: rel_h,
                        brick: unit_brick,
                    })
                }
            }

            slices.push(Chunk {
                unit_brick,
                color,
                l: min_l,
                w: min_w,
                h: slice_h,
                length: chunk_length,
                width: chunk_width,
                height,
                ws_included,
                bricks,
            });
            slice_h += height;
        }

        slices
    }
}

pub struct TexturedMosaic<B, C> {
    mosaic: Mosaic<B, C>
}

impl<B: Brick, C: Color> TexturedMosaic<B, C> {
    pub fn reduce_bricks(self, bricks: &[B]) -> Self {
        TexturedMosaic { mosaic: self.mosaic.reduce_bricks(bricks) }
    }
}

// ====================
// PRIVATE TYPE ALIASES
// ====================

type RawColor = Srgba<u8>;

type HeightMap = Pixels<u16>;

// ====================
// PRIVATE FUNCTIONS
// ====================

fn visited_index(l: u16, w: u16, h: u16, length: u16, width: u16) -> usize {
    h as usize * length as usize * width as usize + w as usize * length as usize + l as usize
}

fn was_visited(visited: &BoolVec, l: u16, w: u16, h: u16, length: u16, width: u16) -> bool {
    visited.get(visited_index(l, w, h, length, width)).unwrap()
}

fn is_new_pos<B: Brick, C: Color>(visited: &BoolVec,
                                  mut brick_fn: impl FnMut(u16, u16, u16, C) -> B,
                                  color_fn: impl Fn(u16, u16) -> C,
                                  l: u16,
                                  w: u16,
                                  h: u16,
                                  length: u16,
                                  width: u16,
                                  start_brick: B,
                                  start_color: C) -> bool {
    !was_visited(visited, l, w, h, length, width) && brick_fn(l, w, h, start_color) == start_brick && color_fn(l, w) == start_color
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
    brick: B
}

impl<B: Brick> AreaSortedBrick<B> {
    fn length(&self) -> u8 {
        self.brick.length()
    }

    fn width(&self) -> u8 {
        self.brick.width()
    }

    fn area(&self) -> u16 {
        self.length() as u16 * self.width() as u16
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
    l: u16,
    w: u16,
    brick: B
}

struct Chunk<B, C> {
    unit_brick: B,
    color: C,
    l: u16,
    w: u16,
    h: u16,
    length: u16,
    width: u16,
    height: u16,
    ws_included: Vec<BTreeSet<u16>>,
    bricks: Vec<PlacedBrick<B>>
}

impl<B: Brick, C: Color> Chunk<B, C> {

    fn reduce_bricks(self, bricks_by_height: &BTreeMap<u16, Vec<AreaSortedBrick<B>>>) -> Self {
        let mut last_h_index = 0;
        let mut remaining_height = self.height;
        let mut layers = Vec::new();

        /* For simplicity, divide the chunk along the h axis into as few layers as possible.
           Because every entry contains at least one 1x1 brick with the given height, we know we
           can fill a layer of that height completely. The standard 1x1 brick is 5 plates tall,
           so most of the time, solutions from a simpler algorithm that only needs to fill 2D
           space should be fairly close to those from an algorithm that considered 3D space. */
        for &height in bricks_by_height.keys().rev() {
            let layers_of_size = remaining_height / height;
            remaining_height %= height;

            for _ in 0..layers_of_size {
                layers.push((height, last_h_index));
                last_h_index += height;
            }
        }

        let bricks: Vec<PlacedBrick<B>> = layers.into_iter().flat_map(|(height, h_index)| {
            let sizes = &bricks_by_height[&height];
            Chunk::<B, C>::reduce_single_layer(sizes, self.length, self.ws_included.clone())
                .into_iter()
                .map(move |placed_brick| PlacedBrick {
                    l: self.l + placed_brick.l,
                    w: self.w + placed_brick.w,
                    h: h_index,
                    brick: placed_brick.brick
                })
        }).collect();

        Chunk {
            unit_brick: self.unit_brick,
            color: self.color,
            l: self.l,
            w: self.w,
            h: self.h,
            length: self.length,
            width: self.width,
            height: self.height,
            ws_included: self.ws_included,
            bricks,
        }
    }

    fn reduce_single_layer(sizes: &[AreaSortedBrick<B>], length: u16, mut ws_included_by_l: Vec<BTreeSet<u16>>) -> Vec<LayerPlacedBrick<B>> {
        let mut bricks = Vec::new();

        /* For every space in the chunk that is empty, try to fit the largest possible brick in
           that space and the spaces surrounding it. If it fits, place the brick at that position
           to fill those empty spaces. This greedy approach may produce sub-optimal solutions, but
           its solutions are often optimal or close to optimal. The problem of finding an optimal
           solution is likely NP-complete, given its similarity to the exact cover problem, and
           thus no known polynomial-time optimal algorithm exists. */
        for l in 0..length {
            let l_index = l as usize;

            while !ws_included_by_l[l_index].is_empty() {
                let ws_included = &ws_included_by_l[l_index];

                if let Some(&w) = ws_included.first() {
                    for size in sizes {
                        if Chunk::<B, C>::fits(l, w, size.length(), size.width(), &ws_included_by_l) {
                            Chunk::<B, C>::remove_brick(l, w, size.length(), size.width(), &mut ws_included_by_l);
                            bricks.push(LayerPlacedBrick {
                                brick: size.brick,
                                l,
                                w
                            })
                        }
                    }
                }
            }
        }


        bricks
    }

    fn fits(l: u16, w: u16, length: u8, width: u8, ws_included_by_l: &[BTreeSet<u16>]) -> bool {
        if u16::MAX - (length as u16) < l || u16::MAX - (width as u16) < w {
            return false;
        }

        let max_l = l + length as u16;
        let max_w = w + width as u16;

        // Brick extends beyond the chunk's length
        if max_l as usize > ws_included_by_l.len() {
            return false;
        }

        // Check whether every point in the chunk that would be filled by the brick is empty
        for test_l in l..max_l {
            if ws_included_by_l[test_l as usize].range(w..max_w).count() < width as usize {
                return false;
            }
        }

        true
    }

    fn remove_brick(l: u16, w: u16, length: u8, width: u8, ws_included_by_l: &mut [BTreeSet<u16>]) {
        let min_l = l as usize;
        let max_l = l as usize + length as usize;
        let max_w = w + width as u16;

        // Remove all entries corresponding to a point inside the brick
        for ws_included in ws_included_by_l.iter_mut().take(max_l).skip(min_l) {
            for cur_w in w..max_w {
                ws_included.remove(&cur_w);
            }
        }

    }
}

struct Pixels<T> {
    values_by_row: Vec<T>,
    length: usize
}

impl<T: Copy> Pixels<T> {
    fn from_fn(mut f: impl FnMut(usize, usize) -> T, length: usize, width: usize) -> Self {
        let mut values_by_row = Vec::new();

        for w in 0..width {
            for l in 0..length {
                values_by_row.push(f(l, w));
            }
        }

        Pixels { values_by_row, length }
    }

    fn value(&self, l: usize, w: usize) -> T {
        self.values_by_row[w * self.length + l]
    }
}

impl<T: Ord> Pixels<T> {
    fn max(&self) -> Option<&T> {
        self.values_by_row.iter().max()
    }
}

impl From<&DynamicImage> for Pixels<RawColor> {
    fn from(image: &DynamicImage) -> Self {
        let length = image.width() as usize;
        let width = image.height() as usize;
        let mut colors = Vec::with_capacity(length * width);

        for w in 0..width {
            for l in 0..length {
                let color = image.get_pixel(l as u32, w as u32).to_rgba();
                let channels = color.channels();
                let red = channels[0];
                let green = channels[1];
                let blue = channels[2];
                let alpha = channels[3];

                colors.push(Srgba::new(red, green, blue, alpha));
            }
        }

        Pixels { values_by_row: colors, length }
    }
}

impl Pixels<RawColor> {
    fn with_palette<C: Color>(self, palette: &[C]) -> Pixels<C> {
        let new_colors = self.values_by_row.into_iter()
            .map(|color| Self::find_similar_color(color, palette))
            .collect();
        Pixels { values_by_row: new_colors, length: self.length }
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

#[cfg(test)]
mod tests {
    use std::hash::Hasher;
    use image::{Rgba, RgbaImage};
    use image::DynamicImage::ImageRgba8;
    use super::*;

    #[derive(Clone, Copy, Eq, PartialEq)]
    pub struct TestBrick<'a> {
        id: &'a str,
        rotation_count: u8,
        length: u8,
        width: u8,
        height: u8,
        unit_brick: Option<&'a TestBrick<'a>>
    }

    impl Hash for TestBrick<'_> {
        fn hash<H: Hasher>(&self, state: &mut H) {
            state.write(self.id.as_bytes());
            state.write_u8(self.rotation_count);
        }
    }

    impl Brick for TestBrick<'_> {
        fn length(&self) -> u8 {
            self.length
        }

        fn width(&self) -> u8 {
            self.width
        }

        fn height(&self) -> u8 {
            self.height
        }

        fn unit_brick(&self) -> Self {
            match self.unit_brick {
                None => *self,
                Some(unit_brick) => *unit_brick
            }
        }

        fn rotate_90(&self) -> Self {
            TestBrick {
                id: self.id,
                rotation_count: (self.rotation_count + 1) % 4,
                length: self.width,
                width: self.length,
                height: self.height,
                unit_brick: self.unit_brick,
            }
        }
    }

    const UNIT_BRICK: TestBrick = TestBrick {
        id: "1x1x1",
        rotation_count: 0,
        length: 1,
        width: 1,
        height: 1,
        unit_brick: None,
    };

    const UNIT_BRICK_2: TestBrick = TestBrick {
        id: "1x1x1_2",
        rotation_count: 0,
        length: 1,
        width: 1,
        height: 1,
        unit_brick: None,
    };

    #[derive(Copy, Clone, Debug, Eq)]
    pub struct TestColor {
        value: Srgba<u8>
    }

    impl TestColor {
        pub const fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
            TestColor { value: Srgba::new(red, green, blue, alpha), }
        }
    }

    impl Default for TestColor {
        fn default() -> Self {
            TestColor::new(0, 0, 0, 0)
        }
    }

    impl PartialEq<Self> for TestColor {
        fn eq(&self, other: &Self) -> bool {
            self.value == other.value
        }
    }

    impl Hash for TestColor {
        fn hash<H: Hasher>(&self, state: &mut H) {
            state.write_u8(self.value.red);
            state.write_u8(self.value.green);
            state.write_u8(self.value.blue);
            state.write_u8(self.value.alpha);
        }
    }

    impl From<TestColor> for Srgba<u8> {
        fn from(color: TestColor) -> Self {
            color.value
        }
    }

    impl Color for TestColor {}

    fn color_to_srgba(color: &Rgba<u8>) -> TestColor {
        TestColor::new(color.0[0], color.0[1], color.0[2], color.0[3])
    }

    fn vec_to_srgba(colors: Vec<Rgba<u8>>) -> Vec<TestColor> {
        colors.into_iter()
            .map(|color| TestColor::new(color.0[0], color.0[1], color.0[2], color.0[3]))
            .collect()
    }

    fn assert_colors_match_img(img: &RgbaImage, chunk: &Chunk<TestBrick, TestColor>) {
        for l in 0..chunk.length {
            for &w in &chunk.ws_included[l as usize] {
                assert_eq!(color_to_srgba(&img.get_pixel((l + chunk.l) as u32, (w + chunk.w) as u32)), chunk.color);
            }
        }
    }

    fn make_test_img() -> (RgbaImage, Vec<TestColor>) {
        let color1 = Rgba([235, 64, 52, 255]);
        let color2 = Rgba([235, 232, 52, 255]);
        let color3 = Rgba([52, 235, 55, 255]);
        let color4 = Rgba([52, 147, 235, 255]);
        let mut img = RgbaImage::new(4, 5);

        img.put_pixel(0, 0, color1);
        img.put_pixel(1, 0, color1);
        img.put_pixel(2, 0, color1);
        img.put_pixel(3, 0, color4);

        img.put_pixel(0, 1, color1);
        img.put_pixel(1, 1, color4);
        img.put_pixel(2, 1, color4);
        img.put_pixel(3, 1, color4);

        img.put_pixel(0, 2, color4);
        img.put_pixel(1, 2, color4);
        img.put_pixel(2, 2, color4);
        img.put_pixel(3, 2, color2);

        img.put_pixel(0, 3, color3);
        img.put_pixel(1, 3, color3);
        img.put_pixel(2, 3, color3);
        img.put_pixel(3, 3, color3);

        img.put_pixel(0, 4, color4);
        img.put_pixel(1, 4, color3);
        img.put_pixel(2, 4, color3);
        img.put_pixel(3, 4, color3);

        let palette = vec_to_srgba(vec![color1, color2, color3, color4]);

        (img, palette)
    }

    #[test]
    fn test_empty_mosaic() {
        let (_, palette) = make_test_img();

        let mosaic = Mosaic::from_image(
            &ImageRgba8(RgbaImage::new(0, 0)),
            &palette[..],
            |_, _, _| 1,
            |_, _, _, _| UNIT_BRICK
        );

        assert_eq!(0, mosaic.chunks.len());
    }

    #[test]
    fn test_height_all_zeroes() {
        let (img, palette) = make_test_img();

        let mosaic = Mosaic::from_image(
            &ImageRgba8(img.clone()),
            &palette[..],
            |_, _, _| 0,
            |_, _, _, _| UNIT_BRICK
        );

        assert_eq!(0, mosaic.chunks.len());
    }

    #[test]
    fn test_height_all_ones() {
        let (img, palette) = make_test_img();

        let mosaic = Mosaic::from_image(
            &ImageRgba8(img.clone()),
            &palette[..],
            |_, _, _| 1,
            |_, _, _, _| UNIT_BRICK
        );

        assert_eq!(5, mosaic.chunks.len());
        let mut total_bricks = 0;
        for chunk in mosaic.chunks {
            assert_eq!(1, chunk.height);
            assert_colors_match_img(&img, &chunk);
            total_bricks += chunk.bricks.len();

            chunk.bricks.iter().for_each(|brick| {
                assert_unit_brick(brick.brick);
                assert_eq!(0, brick.h);
            });
        }
        assert_eq!(4 * 5, total_bricks);
    }

    #[test]
    fn test_height_all_twos() {
        let (img, palette) = make_test_img();

        let mosaic = Mosaic::from_image(
            &ImageRgba8(img.clone()),
            &palette[..],
            |_, _, _| 2,
            |_, _, _, _| UNIT_BRICK
        );

        assert_eq!(5, mosaic.chunks.len());
        let mut total_bricks = 0;
        for chunk in mosaic.chunks {
            assert_eq!(2, chunk.height);
            assert_colors_match_img(&img, &chunk);
            total_bricks += chunk.bricks.len();

            chunk.bricks.iter().for_each(|brick| {
                assert_unit_brick(brick.brick);
                assert!(brick.h == 0 || brick.h == 1);
            });
        }
        assert_eq!(4 * 5 * 2, total_bricks);
    }

    #[test]
    fn test_height_varied() {
        let (img, palette) = make_test_img();

        let heights = [
            [5, 2, 1, 1],
            [5, 5, 2, 2],
            [1, 0, 3, 2],
            [4, 3, 1, 2],
            [3, 1, 1, 4]
        ];
        let expected_total_bricks: u16 = heights.iter()
            .map(|row| row.iter().sum::<u16>())
            .sum();

        let mosaic = Mosaic::from_image(
            &ImageRgba8(img.clone()),
            &palette[..],
            |l, w, _| heights[w as usize][l as usize],
            |_, _, _, _| UNIT_BRICK
        );

        let mut total_bricks = 0;
        for chunk in mosaic.chunks {
            assert_colors_match_img(&img, &chunk);
            total_bricks += chunk.bricks.len();
        }
        assert_eq!(expected_total_bricks as usize, total_bricks);
    }

    #[test]
    fn test_bricks_and_height_varied() {
        let (img, palette) = make_test_img();

        let heights = [
            [5, 2, 1, 1],
            [5, 5, 2, 2],
            [1, 0, 3, 2],
            [4, 3, 1, 2],
            [3, 1, 1, 4]
        ];
        let expected_total_bricks_even: u16 = heights.iter().enumerate()
            .filter(|(index, _)| index % 2 == 0)
            .map(|(_, row)| row)
            .map(|row| row.iter().sum::<u16>())
            .sum();
        let expected_total_bricks_odd: u16 = heights.iter().enumerate()
            .filter(|(index, _)| index % 2 == 1)
            .map(|(_, row)| row)
            .map(|row| row.iter().sum::<u16>())
            .sum();

        let mosaic = Mosaic::from_image(
            &ImageRgba8(img.clone()),
            &palette[..],
            |l, w, _| heights[w as usize][l as usize],
            |_, w, _, _| match w % 2 == 0 {
                true => UNIT_BRICK_2,
                false => UNIT_BRICK
            }
        );

        let mut total_bricks_even = 0;
        let mut total_bricks_odd = 0;
        for chunk in mosaic.chunks {
            assert_colors_match_img(&img, &chunk);

            if chunk.w % 2 == 0 {
                total_bricks_even += chunk.bricks.len();
            } else {
                total_bricks_odd += chunk.bricks.len();
            }
        }
        assert_eq!(expected_total_bricks_even as usize, total_bricks_even);
        assert_eq!(expected_total_bricks_odd as usize, total_bricks_odd);
    }

    #[test]
    fn test_empty_palette() {
        let (img, _) = make_test_img();

        let mosaic = Mosaic::from_image(
            &ImageRgba8(img.clone()),
            &[],
            |_, _, _| 1,
            |_, _, _, _| UNIT_BRICK
        );

        assert_eq!(1, mosaic.chunks.len());
        let mut total_bricks = 0;
        for chunk in mosaic.chunks {
            assert_eq!(1, chunk.height);
            total_bricks += chunk.bricks.len();
            assert_eq!(TestColor::new(0, 0, 0, 0), chunk.color);

            chunk.bricks.iter().for_each(|brick| {
                assert_unit_brick(brick.brick);
                assert_eq!(0, brick.h);
            });
        }
        assert_eq!(4 * 5, total_bricks);
    }
}
