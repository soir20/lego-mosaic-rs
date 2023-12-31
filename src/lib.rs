#[cfg(feature = "palette")]
pub mod palette;

#[cfg(feature = "image")]
pub mod image;

#[cfg(feature = "ldraw")]
pub mod ldraw;

mod base;

pub use base::*;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashMap, VecDeque};
use std::hash::Hash;
use boolvec::BoolVec;

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
// PUBLIC TYPE ALIASES
// ====================

pub type RawColor = Srgba<u8>;

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

pub trait Image {
    fn pixel(&self, l: u32, w: u32) -> RawColor;

    fn length(&self) -> u32;

    fn width(&self) -> u32;
}

pub trait Palette<C> {
    fn nearest(&self, color: RawColor) -> Option<C>;
}

// ====================
// PUBLIC STRUCTS
// ====================

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Srgba<T> {
    red: T,
    green: T,
    blue: T,
    alpha: T
}

impl<T> Srgba<T> {
    pub fn red(&self) -> &T {
        &self.red
    }

    pub fn green(&self) -> &T {
        &self.green
    }

    pub fn blue(&self) -> &T {
        &self.blue
    }

    pub fn alpha(&self) -> &T {
        &self.alpha
    }
}

impl Srgba<u8> {
    pub const fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Srgba<u8> {
        Srgba {
            red,
            green,
            blue,
            alpha
        }
    }
}

#[non_exhaustive]
#[derive(Debug, Eq, PartialEq)]
pub enum MosaicError<B> {
    NotUnitBrick(B),
    PointerTooSmall
}

pub struct PlacedBrick<B, C> {
    l: u32,
    w: u32,
    h: u32,
    brick: B,
    color: C
}

impl<B: Copy, C: Copy> PlacedBrick<B, C> {
    pub fn l(&self) -> u32 {
        self.l
    }

    pub fn w(&self) -> u32 {
        self.w
    }

    pub fn h(&self) -> u32 {
        self.h
    }

    pub fn brick(&self) -> B {
        self.brick
    }

    pub fn color(&self) -> C {
        self.color
    }
}

#[derive(Debug)]
pub struct Mosaic<B, C> {
    sections: Vec<Section<B, C>>,
    length: u32,
    width: u32
}

impl<B: Brick, C: Color> Mosaic<B, C> {
    pub fn from_image<I: Image>(image: &I,
                                palette: &impl Palette<C>,
                                mut height_fn: impl FnMut(u32, u32, C) -> u32,
                                mut brick_fn: impl FnMut(u32, u32, u32, C) -> B) -> Result<Self, MosaicError<B>> {
        let section_size = u8::MAX as u32;
        let section_images = Mosaic::<B, C>::make_sections::<I>(image, section_size);
        let mut sections = Vec::with_capacity(section_images.len());

        /* Dividing the mosaic into sections allows u8s to be used for brick coordinates,
           significantly reducing memory required. It also limits memory to the amount required
           for the section while the mosaic is being generated and improves spatial locality. */
        for (section_l, section_w, section_length, section_width) in section_images {

            // Cache colors, heights, and bricks so functions are only called once per point
            let raw_colors: Pixels<RawColor> = Pixels::<RawColor>::from_fn(
                |l, w| image.pixel(l as u32 + section_l, w as u32 + section_w),
                section_length as usize,
                section_width as usize
            );
            let colors = raw_colors.with_palette(palette);

            let height_map = HeightMap::from_fn(
                |l, w| height_fn(l as u32 + section_l, w as u32 + section_w, colors.value(l, w)),
                section_length as usize,
                section_width as usize
            );
            let max_height = height_map.max().map_or(0, |max| *max);

            let mut section_h = 0;

            while section_h < max_height {
                let section_height = section_size.min(max_height - section_h);
                let mut brick_cache = BTreeMap::new();

                // Build contiguous 3D chunks (with same color and brick) of the mosaic
                let chunks = Mosaic::<B, C>::build_chunks(
                    section_length,
                    section_width,
                    section_height as u8,
                    |l, w| {
                        let height = height_map.value(l as usize, w as usize);
                        match height > section_h {
                            true => section_size.min(height - section_h) as u8,
                            false => 0
                        }
                    },
                    |l, w, h, color| *brick_cache.entry((l, w, h))
                        .or_insert_with(|| brick_fn(
                            l as u32 + section_l,
                            w as u32 + section_w,
                            h as u32 + section_h,
                            color
                        )),
                    |l, w| colors.value(l as usize, w as usize)
                )?;

                sections.push((section_l, section_w, section_h, chunks));

                section_h += section_height;
            }
        }

        Ok(Mosaic::new(sections, image.length(), image.width()))
    }

    pub fn reduce_bricks(self, bricks: &[B]) -> Result<Self, MosaicError<B>> {
        let bricks_by_type: HashMap<B, Vec<VolumeSortedBrick<B>>> = bricks.iter()
            .fold(Ok(HashMap::new()), |partitions_result, &brick| {
                if let Ok(mut partitions) = partitions_result {

                    // Consider each brick's associated unit brick as its type
                    let unit_brick = assert_unit_brick(brick.unit_brick())?;
                    let entry = partitions.entry(unit_brick).or_insert_with(Vec::new);
                    entry.push(VolumeSortedBrick { brick });

                    // A square brick rotated 90 degrees is redundant
                    if brick.length() != brick.width() {
                        entry.push(VolumeSortedBrick { brick: brick.rotate_90() });
                    }

                    Ok(partitions)
                } else {
                    partitions_result
                }
            })?
            .into_iter()
            .map(|(unit_brick, mut bricks)| {

                // Always add the unit brick so at least some bricks are returned
                bricks.push(VolumeSortedBrick { brick: unit_brick });

                // Sort bricks by volume so that larger bricks are chosen first
                bricks.sort();

                (unit_brick, bricks)
            })
            .collect();

        let chunks = self.sections.into_iter()
            .map(|(l, w, h, chunks)| (
                l,
                w,
                h,
                chunks.into_iter().map(|chunk| {
                    if bricks_by_type.contains_key(&chunk.unit_brick) {
                        let bricks_by_height = &bricks_by_type[&chunk.unit_brick];
                        chunk.reduce_bricks(bricks_by_height)
                    } else {
                        chunk
                    }
                }).collect()
            ))
            .collect();

        Ok(Mosaic::new(chunks, self.length, self.width))
    }

    pub fn length(&self) -> u32 {
        self.length
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn iter(&self) -> impl Iterator<Item=PlacedBrick<B, C>> + '_ {
        self.sections.iter().flat_map(|(l, w, h, chunks)|
            chunks.iter().flat_map(move |chunk|
                chunk.bricks.iter().map(move |brick| PlacedBrick {
                    l: l + chunk.l as u32 + brick.l as u32,
                    w: w + chunk.w as u32 + brick.w as u32,
                    h: h + chunk.h as u32 + brick.h as u32,
                    brick: brick.brick,
                    color: chunk.color
                })
            )
        )
    }

    fn new(sections: Vec<Section<B, C>>, length: u32, width: u32) -> Self {
        Mosaic {
            sections: sections.into_iter()
                .filter(|(_, _, _, chunks)| chunks.iter().all(|chunk| chunk.length > 0 && chunk.width > 0 && chunk.height > 0))
                .collect(),
            length,
            width
        }
    }

    fn make_sections<I: Image>(image: &I, section_size: u32) -> Vec<(u32, u32, u8, u8)> {
        let mut section_l = 0;

        let image_length = image.length();
        let image_width = image.width();

        let mut sections = Vec::new();

        while section_l < image_length {
            let section_length = section_size.min(image_length - section_l);

            let mut section_w = 0;
            while section_w < image_width {
                let section_width = section_size.min(image_width - section_w);
                sections.push((section_l, section_w, section_length as u8, section_width as u8));

                section_w += section_width;
            }

            section_l += section_length;
        }

        sections
    }

    fn build_chunks(length: u8,
                    width: u8,
                    max_height: u8,
                    mut height_fn: impl FnMut(u8, u8) -> u8,
                    mut brick_fn: impl FnMut(u8, u8, u8, C) -> B,
                    color_fn: impl Fn(u8, u8) -> C) -> Result<Vec<Chunk<B, C>>, MosaicError<B>> {
        if usize::MAX / length as usize / width as usize / max_height as usize == 0 {
            return Err(MosaicError::PointerTooSmall);
        }

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
                    let start_brick = assert_unit_brick(brick_fn(start_l, start_w, start_h, start_color))?;
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

        Ok(chunks)
    }

    fn slice_chunk(coords_in_chunk: BTreeMap<u8, BTreeSet<(u8, u8)>>,
                   unit_brick: B, color: C, min_l: u8, max_l: u8,
                   min_w: u8, max_w: u8, min_h: u8) -> Vec<Chunk<B, C>> {
        if coords_in_chunk.is_empty() {
            return Vec::new();
        }

        let mut heights = Vec::new();
        let mut height = 0;
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
                    bricks.push(ChunkPlacedBrick {
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

// ====================
// PRIVATE TYPE ALIASES
// ====================

type Section<B, C> = (u32, u32, u32, Vec<Chunk<B, C>>);
type HeightMap = Pixels<u32>;

// ====================
// PRIVATE FUNCTIONS
// ====================

fn visited_index(l: u8, w: u8, h: u8, length: u8, width: u8) -> usize {
    h as usize * length as usize * width as usize + w as usize * length as usize + l as usize
}

fn was_visited(visited: &BoolVec, l: u8, w: u8, h: u8, length: u8, width: u8) -> bool {
    visited.get(visited_index(l, w, h, length, width)).unwrap()
}

fn is_new_pos<B: Brick, C: Color>(visited: &BoolVec,
                                  mut brick_fn: impl FnMut(u8, u8, u8, C) -> B,
                                  color_fn: impl Fn(u8, u8) -> C,
                                  l: u8,
                                  w: u8,
                                  h: u8,
                                  length: u8,
                                  width: u8,
                                  start_brick: B,
                                  start_color: C) -> bool {
    !was_visited(visited, l, w, h, length, width) && brick_fn(l, w, h, start_color) == start_brick && color_fn(l, w) == start_color
}

fn assert_unit_brick<B: Brick>(brick: B) -> Result<B, MosaicError<B>> {
    match brick.length() == 1 && brick.width() == 1 && brick.height() == 1 {
        true => Ok(brick),
        false => Err(MosaicError::NotUnitBrick(brick))
    }
}

// ====================
// PRIVATE STRUCTS
// ====================

struct VolumeSortedBrick<B> {
    brick: B
}

impl<B: Brick> VolumeSortedBrick<B> {
    fn length(&self) -> u8 {
        self.brick.length()
    }

    fn width(&self) -> u8 {
        self.brick.width()
    }

    fn height(&self) -> u8 {
        self.brick.height()
    }

    fn volume(&self) -> u32 {
        self.length() as u32 * self.width() as u32 * self.height() as u32
    }
}

impl<B: Brick> Eq for VolumeSortedBrick<B> {}

impl<B: Brick> PartialEq<Self> for VolumeSortedBrick<B> {
    fn eq(&self, other: &Self) -> bool {
        self.brick == other.brick
    }
}

impl<B: Brick> PartialOrd<Self> for VolumeSortedBrick<B> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<B: Brick> Ord for VolumeSortedBrick<B> {
    fn cmp(&self, other: &Self) -> Ordering {
        let volume1 = self.volume();
        let volume2 = other.volume();

        // Sort in descending order
        volume2.cmp(&volume1)

    }
}

#[derive(Clone, Debug)]
struct ChunkPlacedBrick<B> {
    l: u8,
    w: u8,
    h: u8,
    brick: B
}

#[derive(Debug)]
struct Chunk<B, C> {
    unit_brick: B,
    color: C,
    l: u8,
    w: u8,
    h: u8,
    length: u8,
    width: u8,
    height: u8,
    ws_included: Vec<BTreeSet<u8>>,
    bricks: Vec<ChunkPlacedBrick<B>>
}

impl<B: Brick, C: Color> Chunk<B, C> {

    fn reduce_bricks(self, sizes: &Vec<VolumeSortedBrick<B>>) -> Self {
        let mut ws_included_by_h: Vec<_> = (0..self.height)
            .map(|_| self.ws_included.clone())
            .collect();
        let mut bricks = Vec::new();

        /* For every space in the chunk that is empty, try to fit the largest possible brick in
           that space and the spaces surrounding it. If it fits, place the brick at that position
           to fill those empty spaces. This greedy approach may produce sub-optimal solutions, but
           its solutions are often optimal or close to optimal. The problem of finding an optimal
           solution is likely NP-complete, given its similarity to the exact cover problem, and
           thus no known polynomial-time optimal algorithm exists. */
        for h in 0..self.height {
            let h_index = h as usize;

            for l in 0..self.length {
                let l_index = l as usize;

                while !ws_included_by_h[h_index][l_index].is_empty() {
                    let ws_included = &ws_included_by_h[h_index][l_index];

                    if let Some(&w) = ws_included.first() {
                        for size in sizes {
                            if Chunk::<B, C>::fits(l, w, h, size.length(), size.width(), size.height(), &ws_included_by_h) {
                                Chunk::<B, C>::remove_brick(l, w, h, size.length(), size.width(), size.height(), &mut ws_included_by_h);
                                bricks.push(ChunkPlacedBrick {
                                    l,
                                    w,
                                    h,
                                    brick: size.brick
                                });
                            }
                        }
                    }

                }
            }
        }

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
            bricks
        }
    }

    fn fits(l: u8, w: u8, h: u8, length: u8, width: u8, height: u8, ws_included_by_h: &[Vec<BTreeSet<u8>>]) -> bool {
        if u8::MAX - height < h || u8::MAX - length < l || u8::MAX - width < w {
            return false;
        }

        let max_h = h + height;

        // Brick extends beyond the chunk's height
        if max_h as usize > ws_included_by_h.len() {
            return false;
        }

        for test_h in h..max_h {
            if !Chunk::<B, C>::fits_layer(l, w, length, width, &ws_included_by_h[test_h as usize]) {
                return false;
            }
        }

        true
    }

    fn fits_layer(l: u8, w: u8, length: u8, width: u8, ws_included_by_l: &[BTreeSet<u8>]) -> bool {
        let max_l = l + length;
        let max_w = w + width;

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

    fn remove_brick(l: u8, w: u8, h: u8, length: u8, width: u8, height: u8, ws_included_by_h: &mut [Vec<BTreeSet<u8>>]) {
        let max_h = h + height;
        for h_index in h..max_h {
            Chunk::<B, C>::remove_brick_layer(l, w, length, width, &mut ws_included_by_h[h_index as usize]);
        }
    }

    fn remove_brick_layer(l: u8, w: u8, length: u8, width: u8, ws_included_by_l: &mut [BTreeSet<u8>]) {
        let min_l = l as usize;
        let max_w = w + width;

        // Remove all entries corresponding to a point inside the brick
        for ws_included in ws_included_by_l.iter_mut().skip(min_l).take(length as usize) {
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

impl Pixels<RawColor> {
    fn with_palette<C: Color>(self, palette: &impl Palette<C>) -> Pixels<C> {
        let new_colors = self.values_by_row.into_iter()
            .map(|color| palette.nearest(color).unwrap_or_default())
            .collect();
        Pixels { values_by_row: new_colors, length: self.length }
    }
}

#[cfg(all(test, feature = "default"))]
mod tests {
    use std::hash::Hasher;
    use crate::palette::EuclideanDistancePalette;
    use super::*;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub(crate) struct TestBrick<'a> {
        pub(crate) id: &'a str,
        pub(crate) rotation_count: u8,
        pub(crate) length: u8,
        pub(crate) width: u8,
        pub(crate) height: u8,
        pub(crate) unit_brick: Option<&'a TestBrick<'a>>
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

    pub(crate) const UNIT_BRICK: TestBrick = TestBrick {
        id: "1x1x1",
        rotation_count: 0,
        length: 1,
        width: 1,
        height: 1,
        unit_brick: None,
    };

    pub(crate) const UNIT_BRICK_2: TestBrick = TestBrick {
        id: "1x1x1_2",
        rotation_count: 0,
        length: 1,
        width: 1,
        height: 1,
        unit_brick: None,
    };

    pub(crate) const TWO_BY_ONE_PLATE: TestBrick = TestBrick {
        id: "2x1x1",
        rotation_count: 0,
        length: 2,
        width: 1,
        height: 1,
        unit_brick: Some(&UNIT_BRICK),
    };

    pub(crate) const ONE_BY_TWO_PLATE: TestBrick = TestBrick {
        id: "1x2x1",
        rotation_count: 0,
        length: 1,
        width: 2,
        height: 1,
        unit_brick: Some(&UNIT_BRICK),
    };

    pub(crate) const TWO_BY_TWO_PLATE: TestBrick = TestBrick {
        id: "2x2x1",
        rotation_count: 0,
        length: 2,
        width: 2,
        height: 1,
        unit_brick: Some(&UNIT_BRICK),
    };

    pub(crate) const FOUR_BY_FOUR_PLATE: TestBrick = TestBrick {
        id: "4x4x1",
        rotation_count: 0,
        length: 4,
        width: 4,
        height: 1,
        unit_brick: Some(&UNIT_BRICK),
    };

    pub(crate) const LENGTH_TWO_UNIT_BRICK: TestBrick = TestBrick {
        id: "2x1x1_unit",
        rotation_count: 0,
        length: 2,
        width: 1,
        height: 1,
        unit_brick: None,
    };

    pub(crate) const WIDTH_TWO_UNIT_BRICK: TestBrick = TestBrick {
        id: "1x2x1_unit",
        rotation_count: 0,
        length: 1,
        width: 2,
        height: 1,
        unit_brick: None,
    };

    pub(crate) const HEIGHT_TWO_UNIT_BRICK: TestBrick = TestBrick {
        id: "2x1x1",
        rotation_count: 0,
        length: 1,
        width: 1,
        height: 2,
        unit_brick: None,
    };

    #[derive(Copy, Clone, Debug, Eq)]
    pub(crate) struct TestColor {
        value: Srgba<u8>
    }

    impl TestColor {
        pub(crate) const fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
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

    struct TestImage {
        colors: Pixels<RawColor>,
        length: u32,
        width: u32
    }

    impl<T> Pixels<T> {
        fn value_mut(&mut self, l: usize, w: usize) -> &mut T {
            &mut self.values_by_row[w * self.length + l]
        }
    }

    impl TestImage {
        fn new(length: u32, width: u32) -> Self {
            TestImage {
                colors: Pixels {
                    values_by_row: vec![RawColor::new(0, 0, 0, 0); length as usize * width as usize],
                    length: length as usize
                },
                length,
                width
            }
        }

        fn put_pixel(&mut self, l: u32, w: u32, new_pixel: RawColor) {
            *self.colors.value_mut(l as usize, w as usize) = new_pixel
        }
    }

    impl Image for TestImage {
        fn pixel(&self, l: u32, w: u32) -> RawColor {
            self.colors.value(l as usize, w as usize)
        }

        fn length(&self) -> u32 {
            self.length
        }

        fn width(&self) -> u32 {
            self.width
        }
    }

    fn assert_colors_match_img(img: &TestImage, chunk: &Chunk<TestBrick, TestColor>) {
        for l in 0..chunk.length {
            for &w in &chunk.ws_included[l as usize] {
                assert_eq!(img.pixel((l + chunk.l) as u32, (w + chunk.w) as u32), chunk.color.value);
            }
        }
    }

    fn make_test_img() -> (TestImage, impl Palette<TestColor>) {
        let color1 = RawColor::new(235, 64, 52, 255);
        let color2 = RawColor::new(235, 232, 52, 255);
        let color3 = RawColor::new(52, 235, 55, 255);
        let color4 = RawColor::new(52, 147, 235, 255);
        let mut img = TestImage::new(4, 5);

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

        let palette: Vec<TestColor> = vec![color1, color2, color3, color4].into_iter()
            .map(|color| TestColor { value: color })
            .collect();

        (img, EuclideanDistancePalette::new(&palette[..]))
    }

    #[test]
    fn test_empty_mosaic() {
        let (_, palette) = make_test_img();

        let mosaic = Mosaic::from_image(
            &TestImage::new(0, 0),
            &palette,
            |_, _, _| 1,
            |_, _, _, _| UNIT_BRICK
        ).unwrap();

        assert_eq!(0, mosaic.sections.len());
        assert_eq!(0, mosaic.iter().fold(0, |total, _| total + 1));
    }

    #[test]
    fn test_height_all_zeroes() {
        let (img, palette) = make_test_img();

        let mosaic = Mosaic::from_image(
            &img,
            &palette,
            |_, _, _| 0,
            |_, _, _, _| UNIT_BRICK
        ).unwrap();

        assert_eq!(0, mosaic.sections.len());
        assert_eq!(0, mosaic.iter().fold(0, |total, _| total + 1));
    }

    #[test]
    fn test_height_all_ones() {
        let (img, palette) = make_test_img();

        let mosaic = Mosaic::from_image(
            &img,
            &palette,
            |_, _, _| 1,
            |_, _, _, _| UNIT_BRICK
        ).unwrap();

        assert_eq!(1, mosaic.sections.len());
        let mut total_bricks = 0;
        for (l, w, h, chunks) in &mosaic.sections {
            assert_eq!(0, *l);
            assert_eq!(0, *w);
            assert_eq!(0, *h);
            assert_eq!(5, chunks.len());
            for chunk in chunks {
                assert_eq!(1, chunk.height);
                assert_colors_match_img(&img, &chunk);
                total_bricks += chunk.bricks.len();

                chunk.bricks.iter().for_each(|brick| {
                    assert_unit_brick(brick.brick).unwrap();
                    assert_eq!(0, brick.h);
                });
            }
        }
        assert_eq!(4 * 5, total_bricks);
        assert_eq!(total_bricks, mosaic.iter().fold(0, |total, _| total + 1));
    }

    #[test]
    fn test_height_all_twos() {
        let (img, palette) = make_test_img();

        let mosaic = Mosaic::from_image(
            &img,
            &palette,
            |_, _, _| 2,
            |_, _, _, _| UNIT_BRICK
        ).unwrap();

        assert_eq!(1, mosaic.sections.len());
        let mut total_bricks = 0;for (l, w, h, chunks) in &mosaic.sections {
            assert_eq!(0, *l);
            assert_eq!(0, *w);
            assert_eq!(0, *h);
            assert_eq!(5, chunks.len());
            for chunk in chunks {
                assert_eq!(2, chunk.height);
                assert_colors_match_img(&img, &chunk);
                total_bricks += chunk.bricks.len();

                chunk.bricks.iter().for_each(|brick| {
                    assert_unit_brick(brick.brick).unwrap();
                    assert!(brick.h == 0 || brick.h == 1);
                });
            }
        }
        assert_eq!(4 * 5 * 2, total_bricks);
        assert_eq!(total_bricks, mosaic.iter().fold(0, |total, _| total + 1));
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
        let expected_total_bricks: u32 = heights.iter()
            .map(|row| row.iter().sum::<u32>())
            .sum();

        let mosaic = Mosaic::from_image(
            &img,
            &palette,
            |l, w, _| heights[w as usize][l as usize],
            |_, _, _, _| UNIT_BRICK
        ).unwrap();

        let mut total_bricks = 0;for (l, w, h, chunks) in &mosaic.sections {
            assert_eq!(0, *l);
            assert_eq!(0, *w);
            assert_eq!(0, *h);
            for chunk in chunks {
                assert_colors_match_img(&img, &chunk);
                total_bricks += chunk.bricks.len();
            }
        }
        assert_eq!(expected_total_bricks as usize, total_bricks);
        assert_eq!(total_bricks, mosaic.iter().fold(0, |total, _| total + 1));
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
        let expected_total_bricks_even: u32 = heights.iter().enumerate()
            .filter(|(index, _)| index % 2 == 0)
            .map(|(_, row)| row)
            .map(|row| row.iter().sum::<u32>())
            .sum();
        let expected_total_bricks_odd: u32 = heights.iter().enumerate()
            .filter(|(index, _)| index % 2 == 1)
            .map(|(_, row)| row)
            .map(|row| row.iter().sum::<u32>())
            .sum();

        let mosaic = Mosaic::from_image(
            &img,
            &palette,
            |l, w, _| heights[w as usize][l as usize],
            |_, w, _, _| match w % 2 == 0 {
                true => UNIT_BRICK_2,
                false => UNIT_BRICK
            }
        ).unwrap();

        let mut total_bricks_even = 0;
        let mut total_bricks_odd = 0;
        for (l, w, h, chunks) in &mosaic.sections {
            assert_eq!(0, *l);
            assert_eq!(0, *w);
            assert_eq!(0, *h);
            for chunk in chunks {
                assert_colors_match_img(&img, &chunk);

                if chunk.w % 2 == 0 {
                    total_bricks_even += chunk.bricks.len();
                } else {
                    total_bricks_odd += chunk.bricks.len();
                }
            }
        }
        assert_eq!(expected_total_bricks_even as usize, total_bricks_even);
        assert_eq!(expected_total_bricks_odd as usize, total_bricks_odd);
        assert_eq!(total_bricks_even + total_bricks_odd, mosaic.iter().fold(0, |total, _| total + 1));
    }

    #[test]
    fn test_empty_palette() {
        let (img, _) = make_test_img();

        let mosaic = Mosaic::from_image(
            &img,
            &EuclideanDistancePalette::new(&[]),
            |_, _, _| 1,
            |_, _, _, _| UNIT_BRICK
        ).unwrap();

        assert_eq!(1, mosaic.sections.len());
        let mut total_bricks = 0;
        for (l, w, h, chunks) in &mosaic.sections {
            assert_eq!(0, *l);
            assert_eq!(0, *w);
            assert_eq!(0, *h);
            assert_eq!(1, chunks.len());
            for chunk in chunks {
                assert_eq!(1, chunk.height);
                total_bricks += chunk.bricks.len();
                assert_eq!(TestColor::new(0, 0, 0, 0), chunk.color);

                chunk.bricks.iter().for_each(|brick| {
                    assert_unit_brick(brick.brick).unwrap();
                    assert_eq!(0, brick.h);
                });
            }
        }
        assert_eq!(4 * 5, total_bricks);
        assert_eq!(total_bricks, mosaic.iter().fold(0, |total, _| total + 1));
    }

    #[test]
    fn test_unit_brick_bad_length() {
        let (img, palette) = make_test_img();

        assert_eq!(
            MosaicError::NotUnitBrick(LENGTH_TWO_UNIT_BRICK),
            Mosaic::from_image(
                &img,
                &palette,
                |_, _, _| 1,
                |_, _, _, _| LENGTH_TWO_UNIT_BRICK
            ).expect_err("should fail with bad length two unit brick error")
        );
    }

    #[test]
    fn test_unit_brick_bad_width() {
        let (img, palette) = make_test_img();

        assert_eq!(
            MosaicError::NotUnitBrick(WIDTH_TWO_UNIT_BRICK),
            Mosaic::from_image(
                &img,
                &palette,
                |_, _, _| 1,
                |_, _, _, _| WIDTH_TWO_UNIT_BRICK
            ).expect_err("should fail with bad width two unit brick error")
        );
    }

    #[test]
    fn test_unit_brick_bad_height() {
        let (img, palette) = make_test_img();

        assert_eq!(
            MosaicError::NotUnitBrick(HEIGHT_TWO_UNIT_BRICK),
            Mosaic::from_image(
                &img,
                &palette,
                |_, _, _| 1,
                |_, _, _, _| HEIGHT_TWO_UNIT_BRICK
            ).expect_err("should fail with bad height two unit brick error")
        );
    }
}
