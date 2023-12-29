use std::iter;
use crate::{Brick, Color, PlacedBrick};
use crate::BaseError::{NotAOneByOneBrick, NotAPlate, NotATwoByOneBrick, NotATwoByTwoBrick};

// ====================
// PUBLIC STRUCTS
// ====================

#[non_exhaustive]
#[derive(Debug, Eq, PartialEq)]
pub enum BaseError<B> {
    NotAOneByOneBrick(B),
    NotATwoByOneBrick(B),
    NotATwoByTwoBrick(B),
    NotAPlate(B)
}

pub struct Base<B, C> {
    base_bricks: Vec<FilledArea<B>>,
    support_bricks: Vec<FilledArea<B>>,
    color: C,
    length: u32,
    width: u32
}

impl<B: Brick, C: Color> Base<B, C> {

    pub fn new(length: u32, width: u32, color: C, one_by_one: B, two_by_one: B, two_by_two: B, other_bricks: &[B]) -> Result<Base<B, C>, BaseError<B>> {
        if one_by_one.length() != 1 || one_by_one.width() != 1 {
            return Err(NotAOneByOneBrick(one_by_one));
        } else if one_by_one.height() != 1 {
            return Err(NotAPlate(one_by_one));
        }

        let mut two_by_one = two_by_one;
        if two_by_one.length() == 1 && two_by_one.width() == 2 {
            two_by_one = two_by_one.rotate_90();
        } else if two_by_one.length() != 2 && two_by_one.width() != 1 {
            return Err(NotATwoByOneBrick(two_by_one));
        } else if two_by_one.height() != 1 {
            return Err(NotAPlate(two_by_one));
        }

        if two_by_two.length() != 2 || two_by_two.width() != 2 {
            return Err(NotATwoByTwoBrick(two_by_two));
        } else if two_by_two.height() != 1 {
            return Err(NotAPlate(two_by_two));
        }

        let mut even_by_one_bricks = vec![two_by_one];
        let mut one_by_even_bricks = vec![two_by_one.rotate_90()];
        let mut even_by_even_bricks = vec![two_by_two];

        for &brick in other_bricks {
            if brick.height() != 1 {
                return Err(NotAPlate(brick));
            }

            if is_even(brick.length() as u32) && brick.width() == 1 {
                even_by_one_bricks.push(brick);
                one_by_even_bricks.push(brick.rotate_90());
            } else if brick.length() == 1 && is_even(brick.width() as u32) {
                even_by_one_bricks.push(brick.rotate_90());
                one_by_even_bricks.push(brick);
            } else if is_even(brick.length() as u32) && is_even(brick.width() as u32) {
                even_by_even_bricks.push(brick);
                if brick.length() != brick.width() {
                    even_by_even_bricks.push(brick.rotate_90());
                }
            }
        }

        sort_by_area(&mut even_by_one_bricks);
        sort_by_area(&mut even_by_even_bricks);

        let even_length = make_even(length);
        let even_width = make_even(width);
        let mut base_bricks = fill(
            0,
            0,
            even_length,
            even_width,
            0,
            &even_by_even_bricks
        );

        let is_odd_length = length != even_length;
        let is_odd_width = width != even_width;

        if is_odd_length {
            let mut areas_right = fill(
                even_length,
                0,
                1,
                even_width,
                0,
                &one_by_even_bricks
            );

            base_bricks.append(&mut areas_right);
        }

        if is_odd_width {
            let mut areas_below = fill(
                0,
                even_width,
                even_length,
                1,
                0,
                &even_by_one_bricks
            );

            base_bricks.append(&mut areas_below);
        }

        if is_odd_length && is_odd_width {
            base_bricks.push(FilledArea {
                brick: one_by_one,
                l: even_length,
                w: even_width,
                length: 1,
                width: 1
            });
        }

        let support_bricks = base_bricks.iter()
            .flat_map(|base| base.build_supports(one_by_one, two_by_one, two_by_two, other_bricks, length, width).into_iter())
            .collect();

        Ok(Base {
            base_bricks,
            support_bricks,
            color,
            length,
            width
        })
    }

    pub fn iter(&self) -> impl Iterator<Item=PlacedBrick<B, C>> + '_ {
        self.layer_iter(&self.support_bricks, 0).chain(self.layer_iter(&self.base_bricks, 1))
    }

    pub fn length(&self) -> u32 {
        self.length
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        2
    }

    fn layer_iter<'a>(&'a self, bricks: &'a Vec<FilledArea<B>>, h: u32) -> impl Iterator<Item=PlacedBrick<B, C>> + '_ {
        bricks.iter().flat_map(move |area|
            (area.l..(area.l + area.length)).step_by(area.brick.length() as usize).flat_map(move |l|
                (area.w..(area.w + area.width)).step_by(area.brick.width() as usize).map(move |w| PlacedBrick {
                    l,
                    w,
                    h,
                    brick: area.brick,
                    color: self.color,
                })
            )
        )
    }

}

// ====================
// PRIVATE FUNCTIONS
// ====================

fn is_even(n: u32) -> bool {
    n % 2 == 0
}

fn make_even(n: u32) -> u32 {
    n & !1
}

fn sub_at_most(n: u32, amount: u32) -> u32 {
    n - n.min(amount)
}

fn sort_by_area<B: Brick>(bricks: &mut Vec<B>) {
    bricks.sort_by(|brick1, brick2| {
        let area1= brick1.length() as u16 * brick1.width() as u16;
        let area2 = brick2.length() as u16 * brick2.width() as u16;

        // Sort in descending order
        area2.cmp(&area1)

    });
}

fn fill<B: Brick>(min_l: u32, min_w: u32, length: u32, width: u32, min_index: usize, bricks: &[B]) -> Vec<FilledArea<B>> {
    let mut remaining_length = length;
    let mut remaining_width = width;

    let mut new_areas = Vec::new();

    let mut filled_length = 0;
    let mut filled_width = 0;

    let mut index = min_index;
    while index < bricks.len() {
        let brick = bricks[index];
        if brick.length() as u32 <= remaining_length && brick.width() as u32 <= remaining_width {
            remaining_length %= brick.length() as u32;
            remaining_width %= brick.width() as u32;

            filled_length = length - remaining_length;
            filled_width = width - remaining_width;
            new_areas.push(FilledArea {
                brick,
                l: min_l,
                w: min_w,
                length: filled_length,
                width: filled_width
            });

            break;
        }

        index += 1;
    }

    // Fill following regions with next largest brick
    index += 1;

    if filled_length > 0 && remaining_width > 0 {
        let mut areas_below = fill(
            min_l,
            min_w + filled_width,
            filled_length,
            remaining_width,
            index,
            bricks
        );
        new_areas.append(&mut areas_below);
    }

    if remaining_length > 0 && width > 0 {
        let mut areas_right = fill(
            min_l + filled_length,
            min_w,
            remaining_length,
            width,
            index,
            bricks
        );
        new_areas.append(&mut areas_right);
    }

    new_areas
}

// ====================
// PRIVATE STRUCTS
// ====================

#[derive(Copy, Clone)]
struct FilledArea<B> {
    brick: B,
    l: u32,
    w: u32,
    length: u32,
    width: u32
}

impl<B: Brick> FilledArea<B> {
    fn build_supports(&self, one_by_one: B, two_by_one: B, two_by_two: B, other_bricks: &[B], mosaic_length: u32, mosaic_width: u32) -> Vec<FilledArea<B>> {

        // Main algorithm works for bases of size 3x3 or larger
        if mosaic_length < 3 && mosaic_width < 3 {
            return vec![self.clone()];
        } else if mosaic_length == 3 && mosaic_width == 2 {
            return vec![
                FilledArea {
                    brick: two_by_one.rotate_90(),
                    l: 0,
                    w: 0,
                    length: 1,
                    width: 2
                },
                FilledArea {
                    brick: two_by_two,
                    l: 1,
                    w: 0,
                    length: 2,
                    width: 2
                }
            ];
        } else if mosaic_length == 2 && mosaic_width == 3 {
            return vec![
                FilledArea {
                    brick: two_by_one,
                    l: 0,
                    w: 0,
                    length: 2,
                    width: 1
                },
                FilledArea {
                    brick: two_by_two,
                    l: 0,
                    w: 1,
                    length: 2,
                    width: 2
                }
            ];
        }

        let (length_two_bricks, width_two_bricks) = FilledArea::<B>::filter_bricks(
            one_by_one,
            two_by_one,
            two_by_two,
            other_bricks
        );

        let is_leftmost_area = self.l == 0;
        let is_topmost_area = self.w == 0;
        let is_rightmost_area = mosaic_length - self.length - self.l == 0;
        let is_bottommost_area = mosaic_width - self.width - self.w == 0;

        let brick_length = self.brick.length() as u32;
        let brick_width = self.brick.width() as u32;

        /* Supports should extend one stud beyond the bottom and right sides of this
           area to connect to other area(s), unless there are no more areas below or
           to the right. */
        let mut support_width = self.width;
        if is_bottommost_area {
            support_width = sub_at_most(support_width, 2);
        }

        let min_l = self.l + brick_length - 1;
        let min_w = self.w + brick_width - 1;
        let mut max_l = self.l + self.length - 1;
        if is_rightmost_area {
            max_l = sub_at_most(max_l, 2);
        }
        let max_w = self.w + sub_at_most(support_width, 1);

        let mut supports = Vec::new();

        for l in (min_l..=max_l).step_by(brick_length as usize) {

            let mut vertical_supports = fill(
                l,
                self.w + 1,
                2,
                support_width,
                0,
                &length_two_bricks
            );
            supports.append(&mut vertical_supports);

            /* Add horizontal supports between vertical supports without overlap.
               If the brick length is 2 or less, there will be no space between
               vertical supports, so skip generation of horizontal supports. */
            if brick_length > 2 {

                /* horizontal_min_l is at the second l from the left side of the brick.
                   l is at the last l inside the brick, so compute the horizontal_min_l
                   by moving l brick_length - 2 points to the left. */
                let horizontal_min_l = l - (brick_length - 2);

                // horizontal_min_l - 1 == first l from the left side of the brick
                let mut support_length = l - horizontal_min_l;
                let is_rightmost_support = mosaic_length - (horizontal_min_l - 1) - support_length == 0;
                if is_rightmost_support {
                    support_length = sub_at_most(support_length, 2);
                }

                for w in (min_w..=max_w).step_by(brick_width as usize) {
                    let mut horizontal_supports = fill(
                        horizontal_min_l,
                        w,
                        support_length,
                        2,
                        0,
                        &width_two_bricks
                    );
                    supports.append(&mut horizontal_supports);
                }
            }

        }

        // Add border supports
        let needs_left_border = is_leftmost_area;
        let needs_top_border = is_topmost_area;
        let needs_right_border = is_even(mosaic_length) && is_rightmost_area;
        let needs_bottom_border = is_even(mosaic_width) && is_bottommost_area;

        let mut border_bricks = vec![two_by_one, two_by_one.rotate_90(), one_by_one];
        border_bricks.extend(
            other_bricks.iter()
                .filter(|brick| brick.length() == 1 || brick.width() == 1)
                .flat_map(|&brick| iter::once(brick).chain(iter::once(brick.rotate_90())))
        );
        if needs_left_border {
            let mut left_border = fill(
                self.l,
                self.w,
                1,
                self.width,
                0,
                &border_bricks
            );
            supports.append(&mut left_border);
        }

        if needs_top_border {

            // Avoid putting two bricks in the top left corner
            let mut l = self.l;
            let mut border_length = self.length;
            if needs_left_border {
                l += 1;
                border_length -= 1;
            }

            let mut top_border = fill(
                l,
                self.w,
                border_length,
                1,
                0,
                &border_bricks
            );
            supports.append(&mut top_border);
        }

        if needs_right_border {

            // Avoid putting two bricks in the top right corner
            let mut w = self.w;
            let mut border_width = self.width;
            if needs_top_border {
                w += 1;
                border_width -= 1;
            }

            let mut right_border = fill(
                self.l + self.length - 1,
                w,
                1,
                border_width,
                0,
                &border_bricks
            );
            supports.append(&mut right_border);
        }

        if needs_bottom_border {

            // Avoid putting two bricks in the bottom left corner
            let mut l = self.l;
            let mut border_length = self.length;
            if needs_left_border {
                l += 1;
                border_length -= 1;
            }

            // Avoid putting two bricks in the bottom right corner
            if needs_right_border {
                border_length -= 1;
            }

            let mut bottom_border = fill(
                l,
                self.w + self.width - 1,
                border_length,
                1,
                0,
                &border_bricks
            );
            supports.append(&mut bottom_border);
        }

        supports
    }

    fn filter_bricks(one_by_one: B, two_by_one: B, two_by_two: B, other_bricks: &[B]) -> (Vec<B>, Vec<B>) {
        let mut bricks = vec![one_by_one, two_by_one, two_by_one.rotate_90(), two_by_two];
        bricks.extend_from_slice(other_bricks);

        let mut length_two_bricks = Vec::new();
        let mut width_two_bricks = Vec::new();

        for brick in bricks {
            if brick.length() == 2 && is_even(brick.width() as u32) {
                length_two_bricks.push(brick);
                width_two_bricks.push(brick.rotate_90());
            } else if brick.width() == 2 && is_even(brick.length() as u32) {
                length_two_bricks.push(brick.rotate_90());
                width_two_bricks.push(brick);
            }
        }

        sort_by_area(&mut length_two_bricks);
        sort_by_area(&mut width_two_bricks);

        (length_two_bricks, width_two_bricks)
    }
}