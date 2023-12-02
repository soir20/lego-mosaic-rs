mod ldraw;

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::hash::Hash;
use boolvec::BoolVec;
use image::{DynamicImage, GenericImageView, Pixel};
use palette::color_difference::Wcag21RelativeContrast;
use palette::Srgba;

//! This API uses l, w, and h coordinate axes, which refer to length, width, and height,
//! respectively. A brick's length refers to its size along the l axis, a brick's
//! width refers to its size along the w axis, and a brick's height refers to its size
//! along the h axis.
//!
//! From a bird's eye view, increasing l refers to moving east, while increasing w
//! refers to moving south. Increasing h refers to increasing altitude above the plane.
//! This is consistent with image editors, as well as the image crate, which put the
//! origin at the top left.
//!
//! The x, y, and z axes are not used because l, w, and h more clearly map to brick size,
//! and many existing programs have conflicting definitions of what y and z refer to.
//! For example, in the LDraw format, decreasing y is analogous to increasing altitude,
//! and x and z are horizontal axes. In other programs, z is the vertical axis instead
//! of y, and increasing z is analogous to increasing altitude. With the l, w, and h axes,
//! the user must explicitly consider the mapping between this API's axes and another
//! program's x, y, and z axes, rather than assuming the API's axes conform to those of
//! another program.

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

pub struct Mosaic<B, C> {
    chunks: Vec<Chunk<B, C>>
}

impl<B: Brick, C: Color> Mosaic<B, C> {

    pub fn from_image(image: &DynamicImage, palette: &[C], unit_brick: B) -> Self {
        assert_unit_brick(unit_brick);

        let raw_colors: Pixels<RawColor> = image.into();
        let colors = raw_colors.with_palette(palette);

        let area = colors.values_by_row.len();
        let length = colors.length;
        let width = area / length;

        let mut visited = BoolVec::filled_with(area, false);
        let mut coords_to_visit = VecDeque::new();
        let mut chunks = Vec::new();

        /* An iterative breadth-first search that explores contiguous chunks of the mosaic with
           the same color, similar to the classic island-finding problem */
        for start_w in 0..width {
            for start_l in 0..length {
                if was_visited(&visited, start_l, start_w, length) {
                    continue;
                }

                let start_color = colors.value(start_l, start_w);
                coords_to_visit.push_back((start_l, start_w));

                let mut coords_in_chunk = Vec::new();
                let mut min_l = start_l;
                let mut min_w = start_w;
                let mut max_l = start_l;

                while !coords_to_visit.is_empty() {
                    let (l, w) = coords_to_visit.pop_front().unwrap();

                    // Avoid an infinite loop by visiting no point twice
                    if was_visited(&visited, l, w, length) {
                        continue;
                    }
                    visited.set(w * length + l, true);

                    coords_in_chunk.push((l, w));

                    min_l = min_l.min(l);
                    min_w = min_w.min(w);
                    max_l = max_l.max(l);

                    // Add position to the left to explore later
                    if l > 0 && is_new_pos::<C>(&visited, &colors, l - 1, w, length, start_color) {
                        coords_to_visit.push_back((l - 1, w));
                    }

                    // Add position to the right to explore later
                    if l < length - 1 && is_new_pos::<C>(&visited, &colors, l + 1, w, length, start_color) {
                        coords_to_visit.push_back((l + 1, w));
                    }

                    // Add position below to explore later
                    if w > 0 && is_new_pos::<C>(&visited, &colors, l, w - 1, length, start_color) {
                        coords_to_visit.push_back((l, w - 1));
                    }

                    // Add position above to explore later
                    if w < width - 1 && is_new_pos::<C>(&visited, &colors, l, w + 1, length, start_color) {
                        coords_to_visit.push_back((l, w + 1));
                    }

                }

                // Compute relative coordinates for every point inside the fully-explored chunk
                let chunk_length = max_l - min_l + 1;
                let mut bricks = Vec::with_capacity(coords_in_chunk.len());
                let mut ws_included = vec![BTreeSet::new(); chunk_length];

                for (l, w) in coords_in_chunk {
                    let rel_l = l - min_l;
                    let rel_w = w - min_w;

                    bricks.push(PlacedBrick {
                        l: rel_l as u16,
                        w: rel_w as u16,
                        h: 0,
                        brick: unit_brick
                    });

                    ws_included[rel_l].insert(rel_w as u16);
                }

                chunks.push(Chunk {
                    unit_brick,
                    color: start_color,
                    l: min_l as u16,
                    w: min_w as u16,
                    h: 0,
                    length: chunk_length as u16,
                    height: 1,
                    ws_included,
                    bricks,
                })
            }
        }

        Mosaic { chunks }
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

        Mosaic { chunks }
    }

    pub fn make_3d(self, height: u16, flip: bool) -> Self {
        let height_map = self.height_map(height, flip);
        Mosaic {
            chunks: self.chunks.into_iter()
                .map(|chunk| {
                    let key = chunk.color;
                    chunk.set_height(height_map[&key])
                })
                .collect()
        }
    }

    fn height_map(&self, height: u16, flip: bool) -> HeightMap<C> {
        if height == 0 {
            return HeightMap::new();
        }

        let (min_luma, max_luma) = self.chunks.iter()
            .map(|chunk| {
                let srgba_f32: Srgba<f32> = chunk.color.into().into_format();
                srgba_f32.relative_luminance().luma
            })
            .fold((1.0f32, 0.0f32), |(min, max), luma| (min.min(luma), max.max(luma)));

        let range = max_luma - min_luma;
        let max_layer_index = height - 1;

        let mut height_map = HeightMap::new();

        self.chunks.iter().for_each(|chunk| {
            let color = chunk.color;
            let entry = height_map.entry(color);
            entry.or_insert_with(|| {
                let srgba_f32: Srgba<f32> = color.into().into_format();
                let luma = srgba_f32.relative_luminance().luma;

                // Normalize the luma within the range of luma values found in the image
                let mut range_rel_luma = (luma - min_luma) / range;
                range_rel_luma = if flip { 1.0 - range_rel_luma } else { range_rel_luma };

                /* height must be u16 because the max integer a 32-bit float can represent
                   exactly is 2^24 + 1 (more than u16::MAX but less than u32::MAX). Add one
                   because every layer must be at least 1 plate tall, while index starts at
                   0. */
                (range_rel_luma * max_layer_index as f32).round() as u16 + 1

            });
        });

        height_map
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

}

// ====================
// PRIVATE TYPE ALIASES
// ====================

type RawColor = Srgba<u8>;

type HeightMap<C> = HashMap<C, u16>;

// ====================
// PRIVATE FUNCTIONS
// ====================

fn was_visited(visited: &BoolVec, l: usize, w: usize, length: usize) -> bool {
    visited.get(w * length + l).unwrap()
}

fn is_new_pos<C: Color>(visited: &BoolVec, colors: &Pixels<C>, l: usize, w: usize, length: usize, start_color: C) -> bool {
    !was_visited(visited, l, w, length) && colors.value(l, w) == start_color
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
    height: u16,
    ws_included: Vec<BTreeSet<u16>>,
    bricks: Vec<PlacedBrick<B>>
}

impl<B: Brick, C: Color> Chunk<B, C> {

    pub fn set_height(mut self, new_height: u16) -> Self {

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

    pub fn reduce_bricks(self, bricks_by_height: &BTreeMap<u16, Vec<AreaSortedBrick<B>>>) -> Self {
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
    pub fn value(&self, l: usize, w: usize) -> T {
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
    pub fn with_palette<C: Color>(self, palette: &[C]) -> Pixels<C> {
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
