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
        let l_size = colors.l_size;
        let w_size = area / l_size;

        let mut visited = BoolVec::filled_with(area, false);
        let mut coords_to_visit = VecDeque::new();
        let mut chunks = Vec::new();

        for start_w in 0..w_size {
            for start_l in 0..l_size {
                if was_visited(&visited, start_l, start_w, l_size) {
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

                    if was_visited(&visited, l, w, l_size) {
                        continue;
                    }
                    visited.set(w * l_size + l, true);

                    coords_in_chunk.push((l, w));

                    min_l = min_l.min(l);
                    min_w = min_w.min(w);
                    max_l = max_l.max(l);

                    if l > 0 && is_new_pos::<C>(&visited, &colors, l - 1, w, l_size, start_color) {
                        coords_to_visit.push_back((l - 1, w));
                    }

                    if l < l_size - 1 && is_new_pos::<C>(&visited, &colors, l + 1, w, l_size, start_color) {
                        coords_to_visit.push_back((l + 1, w));
                    }

                    if w > 0 && is_new_pos::<C>(&visited, &colors, l, w - 1, l_size, start_color) {
                        coords_to_visit.push_back((l, w - 1));
                    }

                    if w < w_size - 1 && is_new_pos::<C>(&visited, &colors, l, w + 1, l_size, start_color) {
                        coords_to_visit.push_back((l, w + 1));
                    }
                }

                let chunk_l_size = max_l - min_l + 1;
                let mut bricks = Vec::with_capacity(coords_in_chunk.len());
                let mut ws_included = vec![BTreeSet::new(); chunk_l_size];

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
                    l_size: chunk_l_size as u16,
                    h_size: 1,
                    ws_included,
                    bricks,
                })
            }
        }

        Mosaic { chunks }
    }

    pub fn reduce_bricks(self, bricks: &[B]) -> Self {
        let bricks_by_h_size: HashMap<B, BTreeMap<u16, Vec<AreaSortedBrick<B>>>> = bricks.iter()
            .fold(HashMap::new(), |mut partitions, &brick| {
                let unit_brick = assert_unit_brick(brick.unit_brick());
                let entry = partitions.entry(unit_brick).or_insert_with(Vec::new);
                entry.push(brick);

                if brick.length() != brick.width() {
                    entry.push(brick.rotate_90());
                }

                partitions
            })
            .into_iter()
            .map(|(unit_brick, bricks)| (unit_brick, Mosaic::<B, C>::partition_by_h_size(bricks)))
            .collect();

        let chunks = self.chunks.into_iter()
            .map(|chunk| {
                let bricks_by_h_size = &bricks_by_h_size[&chunk.unit_brick];
                chunk.reduce_bricks(bricks_by_h_size)
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
                    chunk.set_h_size(height_map[&key])
                })
                .collect()
        }
    }

    fn height_map(&self, h_size: u16, flip: bool) -> HeightMap<C> {
        if h_size == 0 {
            return HeightMap::new();
        }

        let (min_luma, max_luma) = self.chunks.iter()
            .map(|chunk| {
                let srgba_f32: Srgba<f32> = chunk.color.into().into_format();
                srgba_f32.relative_luminance().luma
            })
            .fold((1.0f32, 0.0f32), |(min, max), luma| (min.min(luma), max.max(luma)));

        let range = max_luma - min_luma;
        let max_layer_index = h_size - 1;

        let mut height_map = HeightMap::new();

        self.chunks.iter().for_each(|chunk| {
            let color = chunk.color;
            let entry = height_map.entry(color);
            entry.or_insert_with(|| {
                let srgba_f32: Srgba<f32> = color.into().into_format();
                let luma = srgba_f32.relative_luminance().luma;
                let mut range_rel_luma = (luma - min_luma) / range;
                range_rel_luma = if flip { 1.0 - range_rel_luma } else { range_rel_luma };

                /* h_size must be u16 because the max integer a 32-bit float can represent
                   exactly is 2^24 + 1 (more than u16::MAX but less than u32::MAX). */
                (range_rel_luma * max_layer_index as f32).round() as u16 + 1

            });
        });

        height_map
    }

    fn partition_by_h_size(bricks: Vec<B>) -> BTreeMap<u16, Vec<AreaSortedBrick<B>>> {

        /* Ensure that every h size has at least one 1x1 brick so that we are certain we can fill
           a layer of that h size. */
        bricks.into_iter().fold(BTreeMap::new(), |mut partitions, brick| {
            partitions.entry(brick.height()).or_insert_with(Vec::new).push(brick);
            partitions
        })
            .into_iter()
            .filter(|(_, bricks)| bricks.iter().any(|brick| brick.length() == 1 && brick.width() == 1))
            .map(|(h_size, bricks)| {

                /* Sort bricks by area so that larger bricks are chosen first. We don't need to
                   sort by volume because the brick-filling algorithm only needs to consider 2D
                   space. */
                let mut sizes: Vec<_> = bricks.into_iter()
                    .map(|brick| AreaSortedBrick { brick })
                    .collect();
                sizes.sort();

                (h_size as u16, sizes)
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

fn was_visited(visited: &BoolVec, l: usize, w: usize, l_size: usize) -> bool {
    visited.get(w * l_size + l).unwrap()
}

fn is_new_pos<C: Color>(visited: &BoolVec, colors: &Pixels<C>, l: usize, w: usize, l_size: usize, start_color: C) -> bool {
    !was_visited(visited, l, w, l_size) && colors.value(l, w) == start_color
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
    fn l_size(&self) -> u8 {
        self.brick.length()
    }

    fn w_size(&self) -> u8 {
        self.brick.width()
    }

    fn area(&self) -> u16 {
        self.l_size() as u16 * self.w_size() as u16
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
    l_size: u16,
    h_size: u16,
    ws_included: Vec<BTreeSet<u16>>,
    bricks: Vec<PlacedBrick<B>>
}

impl<B: Brick, C: Color> Chunk<B, C> {

    pub fn set_h_size(mut self, new_h_size: u16) -> Self {

        /* For any column (l, w), any brick at height h in the column will be the same color.
           Hence, we only need to consider the numerical difference in the number of layers and
           remove bricks or move them vertically. */
        let new_layers = new_h_size.abs_diff(self.h_size);

        if self.h_size > new_h_size {
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
            for h in self.h_size..(self.h_size + new_layers) {
                for l in 0..self.l_size {
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

        self.h_size = new_h_size;

        self
    }

    pub fn reduce_bricks(self, bricks_by_h_size: &BTreeMap<u16, Vec<AreaSortedBrick<B>>>) -> Self {
        let mut last_h_index = 0;
        let mut remaining_height = self.h_size;
        let mut layers = Vec::new();

        /* For simplicity, divide the chunk along the h axis into as few layers as possible.
           Because every entry contains at least one 1x1 brick with the given height, we know we
           can fill a layer of that height completely. The standard 1x1 brick is 5 plates tall,
           so most of the time, solutions from a simpler algorithm that only needs to fill 2D
           space should be fairly close to those from an algorithm that considered 3D space. */
        for &h_size in bricks_by_h_size.keys().rev() {
            let layers_of_size = remaining_height / h_size;
            remaining_height %= h_size;

            for _ in 0..layers_of_size {
                layers.push((h_size, last_h_index));
                last_h_index += h_size;
            }
        }

        let bricks: Vec<PlacedBrick<B>> = layers.into_iter().flat_map(|(h_size, h_index)| {
            let sizes = &bricks_by_h_size[&h_size];
            Chunk::<B, C>::reduce_single_layer(sizes, self.l_size, self.ws_included.clone())
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
            l_size: self.l_size,
            h_size: self.h_size,
            ws_included: self.ws_included,
            bricks,
        }
    }

    fn reduce_single_layer(sizes: &[AreaSortedBrick<B>], l_size: u16, mut ws_included_by_l: Vec<BTreeSet<u16>>) -> Vec<LayerPlacedBrick<B>> {
        let mut bricks = Vec::new();

        /* For every space in the chunk that is empty, try to fit the largest possible brick in
           that space and the spaces surrounding it. If it fits, place the brick at that position
           to fill those empty spaces. This greedy approach may produce sub-optimal solutions, but
           its solutions are often optimal or close to optimal. The problem of finding an optimal
           solution is likely NP-complete, given its similarity to the exact cover problem, and
           thus no known polynomial-time optimal algorithm exists. */
        for l in 0..l_size {
            let l_index = l as usize;

            while !ws_included_by_l[l_index].is_empty() {
                let ws_included = &ws_included_by_l[l_index];

                if let Some(&w) = ws_included.first() {
                    for size in sizes {
                        if Chunk::<B, C>::fits(l, w, size.l_size(), size.w_size(), &ws_included_by_l) {
                            Chunk::<B, C>::remove_brick(l, w, size.l_size(), size.w_size(), &mut ws_included_by_l);
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

    fn fits(l: u16, w: u16, l_size: u8, w_size: u8, ws_included_by_l: &[BTreeSet<u16>]) -> bool {
        let max_l = l + l_size as u16;
        let max_w = w + w_size as u16;

        // Brick extends beyond the chunk's length
        if max_l as usize > ws_included_by_l.len() {
            return false;
        }

        // Check whether every point in the chunk that would be filled by the brick is empty
        for test_l in l..max_l {
            if ws_included_by_l[test_l as usize].range(w..max_w).count() < w_size as usize {
                return false;
            }
        }

        true
    }

    fn remove_brick(l: u16, w: u16, l_size: u8, w_size: u8, ws_included_by_l: &mut [BTreeSet<u16>]) {
        let min_l = l as usize;
        let max_l = l as usize + l_size as usize;
        let max_w = w + w_size as u16;

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
    l_size: usize
}

impl<T: Copy> Pixels<T> {
    pub fn value(&self, l: usize, w: usize) -> T {
        self.values_by_row[w * self.l_size + l]
    }
}

impl From<&DynamicImage> for Pixels<RawColor> {
    fn from(image: &DynamicImage) -> Self {
        let l_size = image.width() as usize;
        let w_size = image.height() as usize;
        let mut colors = Vec::with_capacity(l_size * w_size);

        for w in 0..w_size {
            for l in 0..l_size {
                let color = image.get_pixel(l as u32, w as u32).to_rgba();
                let channels = color.channels();
                let red = channels[0];
                let green = channels[1];
                let blue = channels[2];
                let alpha = channels[3];

                colors.push(Srgba::new(red, green, blue, alpha));
            }
        }

        Pixels { values_by_row: colors, l_size }
    }
}

impl Pixels<RawColor> {
    pub fn with_palette<C: Color>(self, palette: &[C]) -> Pixels<C> {
        let new_colors = self.values_by_row.into_iter()
            .map(|color| Self::find_similar_color(color, palette))
            .collect();
        Pixels { values_by_row: new_colors, l_size: self.l_size }
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
