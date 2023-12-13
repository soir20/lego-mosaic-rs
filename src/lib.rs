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
                      unit_brick: B,
                      mut height_fn: impl FnMut(u16, u16, C) -> u16) -> Self {
        let raw_colors: Pixels<RawColor> = image.into();
        let colors = raw_colors.with_palette(palette);
        let length = colors.length;
        let width = colors.values_by_row.len() / colors.length;

        let mut chunks = Mosaic::<B, C>::build_2d_chunks(
            length,
            width,
            0,
            0,
            0,
            |_, _| unit_brick,
            |l, w| colors.value(l, w),
            |_, _| false
        );

        chunks = Mosaic::<B, C>::build_3d_chunks(
            chunks,
            length,
            width,
            |l, w| height_fn(l, w, colors.value(l as usize, w as usize))
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

    fn build_2d_chunks(region_length: usize,
                       region_width: usize,
                       region_l: u16,
                       region_w: u16,
                       region_h: u16,
                       mut bricks: impl FnMut(usize, usize) -> B,
                       colors: impl Fn(usize, usize) -> C,
                       is_empty: impl Fn(usize, usize) -> bool) -> Vec<Chunk<B, C>> {
        let mut visited = BoolVec::filled_with(region_length * region_width, false);
        let mut coords_to_visit = VecDeque::new();
        let mut chunks = Vec::new();

        /* An iterative breadth-first search that explores contiguous chunks of the mosaic with
           the same brick type and color, similar to the classic island-finding problem */
        for start_w in 0..region_width {
            for start_l in 0..region_length {
                if was_visited(&visited, start_l, start_w, region_length, &is_empty) {
                    continue;
                }

                let start_brick = assert_unit_brick(bricks(start_l, start_w));
                let start_color = colors(start_l, start_w);
                coords_to_visit.push_back((start_l, start_w));

                let mut coords_in_chunk = Vec::new();
                let mut min_l = start_l;
                let mut min_w = start_w;
                let mut max_l = start_l;
                let mut max_w = start_w;

                while !coords_to_visit.is_empty() {
                    let (l, w) = coords_to_visit.pop_front().unwrap();

                    // Avoid an infinite loop by visiting no point twice
                    if was_visited(&visited, l, w, region_length, &is_empty) {
                        continue;
                    }
                    visited.set(w * region_length + l, true);

                    coords_in_chunk.push((l, w));

                    min_l = min_l.min(l);
                    min_w = min_w.min(w);
                    max_l = max_l.max(l);
                    max_w = max_w.max(w);

                    // Add position to the left to explore later
                    if l > 0 && is_new_pos::<B, C>(&visited, &mut bricks, &colors, &is_empty, l - 1, w, region_length, start_brick, start_color) {
                        coords_to_visit.push_back((l - 1, w));
                    }

                    // Add position to the right to explore later
                    if l < region_length - 1 && is_new_pos::<B, C>(&visited, &mut bricks, &colors, &is_empty, l + 1, w, region_length, start_brick, start_color) {
                        coords_to_visit.push_back((l + 1, w));
                    }

                    // Add position below to explore later
                    if w > 0 && is_new_pos::<B, C>(&visited, &mut bricks, &colors, &is_empty, l, w - 1, region_length, start_brick, start_color) {
                        coords_to_visit.push_back((l, w - 1));
                    }

                    // Add position above to explore later
                    if w < region_width - 1 && is_new_pos::<B, C>(&visited, &mut bricks, &colors, &is_empty, l, w + 1, region_length, start_brick, start_color) {
                        coords_to_visit.push_back((l, w + 1));
                    }

                }

                // Compute relative coordinates for every point inside the fully-explored chunk
                let chunk_length = max_l - min_l + 1;
                let chunk_width = max_w - min_w + 1;
                let mut bricks = Vec::with_capacity(coords_in_chunk.len());
                let mut ws_included = vec![BTreeSet::new(); chunk_length];

                for (l, w) in coords_in_chunk {
                    let rel_l = l - min_l;
                    let rel_w = w - min_w;

                    bricks.push(PlacedBrick {
                        l: rel_l as u16,
                        w: rel_w as u16,
                        h: 0,
                        brick: start_brick
                    });

                    ws_included[rel_l].insert(rel_w as u16);
                }

                if !bricks.is_empty() {
                    chunks.push(Chunk {
                        unit_brick: start_brick,
                        color: start_color,
                        l: region_l + min_l as u16,
                        w: region_w + min_w as u16,
                        h: region_h,
                        length: chunk_length as u16,
                        width: chunk_width as u16,
                        height: 1,
                        ws_included,
                        bricks,
                    })
                }
            }
        }

        chunks
    }

    fn build_3d_chunks(chunks: Vec<Chunk<B, C>>,
                       length: usize,
                       width: usize,
                       mut height: impl FnMut(u16, u16) -> u16) -> Vec<Chunk<B, C>> {
        let height_map = HeightMap::from_fn(
            |l, w| height(l as u16, w as u16),
            length,
            width
        );

        let mut new_chunks = Vec::new();

        for chunk in chunks {
            assert_eq!(0, chunk.h);

            let heights = (0..chunk.length as usize).into_iter()
                .flat_map(|l| chunk.ws_included[l].iter()
                    .map(move |&w| (l, w as usize))
                )
                .map(|(l, w)| height_map.value(chunk.l as usize + l, chunk.w as usize + w))
                .collect::<BTreeSet<_>>();
            let mut last_height = 0;

            for &height in &heights {
                let mut new_chunk = Chunk {
                    unit_brick: chunk.unit_brick,
                    color: chunk.color,
                    l: chunk.l,
                    w: chunk.w,
                    h: last_height,
                    length: chunk.length,
                    width: chunk.width,
                    height: height - last_height,
                    ws_included: Vec::with_capacity(chunk.ws_included.len()),
                    bricks: Vec::with_capacity(chunk.bricks.len()),
                };

                for l in 0..chunk.length {
                    let mut ws_included = BTreeSet::new();

                    for &w in chunk.ws_included[l as usize].iter() {
                        if height_map.value(l as usize, w as usize) >= height {
                            ws_included.insert(w);

                            for h in 0..new_chunk.height {
                                new_chunk.bricks.push(PlacedBrick {
                                    l,
                                    w,
                                    h,
                                    brick: new_chunk.unit_brick,
                                });
                            }
                        }
                    }

                    new_chunk.ws_included.push(ws_included);
                }

                new_chunks.push(new_chunk);
                last_height = height;
            }
        }

        new_chunks
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

fn was_visited(visited: &BoolVec, l: usize, w: usize, length: usize, is_empty: impl Fn(usize, usize) -> bool) -> bool {
    !is_empty(l, w) && visited.get(w * length + l).unwrap()
}

fn is_new_pos<B: Brick, C: Color>(visited: &BoolVec,
                                  mut bricks: impl FnMut(usize, usize) -> B,
                                  colors: impl Fn(usize, usize) -> C,
                                  is_empty: impl Fn(usize, usize) -> bool,
                                  l: usize,
                                  w: usize,
                                  length: usize,
                                  start_brick: B,
                                  start_color: C) -> bool {
    !was_visited(visited, l, w, length, is_empty) && bricks(l, w) == start_brick && colors(l, w) == start_color
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

    fn set_height(mut self, new_height: u16) -> Self {

        /* For any column (l, w), any brick at height h in the column will be the same color.
           Hence, we only need to consider the numerical difference in the number of layers and
           remove bricks or move them vertically. */
        let new_layers = new_height.abs_diff(self.height);

        if self.height > new_height {
            let new_min_h = new_layers;
            self.bricks = self.bricks.into_iter()
                .flat_map(|brick| {

                    /* If the brick's bottom is at or above the threshold, we only need to update
                       its h coordinate relative to the new minimum. */
                    if brick.h >= new_min_h {
                        return vec![PlacedBrick {
                            l: brick.l,
                            w: brick.w,
                            h: brick.h - new_min_h,
                            brick: brick.brick
                        }];
                    }

                    // Remove any bricks entirely below the threshold
                    let brick_h_above = brick.h + brick.brick.height() as u16;
                    if brick_h_above <= new_min_h {
                        return vec![];
                    }

                    /* In this case, the threshold passes through the middle of the brick. Remove
                       it and fill the empty space between the threshold and the brick's top with
                       1x1 bricks plates. Even if the bricks were reduced previously, this method
                       does not guarantee any reduction in bricks. */
                    let hs_to_replace = brick_h_above - new_min_h;
                    let mut new_bricks = Vec::with_capacity(
                        hs_to_replace as usize * brick.brick.length() as usize * brick.brick.width() as usize
                    );

                    for h in 0..(brick_h_above - new_min_h) {
                        for l in brick.l..(brick.l + brick.brick.length() as u16) {
                            for w in brick.w..(brick.w + brick.brick.width() as u16) {
                                new_bricks.push(PlacedBrick {
                                    l,
                                    w,
                                    h,
                                    brick: self.unit_brick
                                });
                            }
                        }
                    }

                    new_bricks
                })
                .collect();
        } else {

            // Fill the new space with 1x1 plates
            for h in self.height..(self.height + new_layers) {
                for l in 0..self.length {
                    for &w in self.ws_included[l as usize].iter() {
                        self.bricks.push(PlacedBrick {
                            l,
                            w,
                            h,
                            brick: self.unit_brick
                        });
                    }
                }
            }

        }

        self.height = new_height;

        self
    }

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
    fn test_height_all_zeroes() {
        let (img, palette) = make_test_img();

        let mosaic = Mosaic::from_image(
            &ImageRgba8(img.clone()),
            &palette[..],
            UNIT_BRICK,
            |_, _, _| 0
        );

        assert_eq!(0, mosaic.chunks.len());
    }

    #[test]
    fn test_height_all_ones() {
        let (img, palette) = make_test_img();

        let mosaic = Mosaic::from_image(
            &ImageRgba8(img.clone()),
            &palette[..],
            UNIT_BRICK,
            |_, _, _| 1
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
            UNIT_BRICK,
            |_, _, _| 2
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
            UNIT_BRICK,
            |l, w, _| heights[w as usize][l as usize]
        );

        let mut total_bricks = 0;
        for chunk in mosaic.chunks {
            assert_colors_match_img(&img, &chunk);
            total_bricks += chunk.bricks.len();
        }
        assert_eq!(expected_total_bricks as usize, total_bricks);
    }
}
