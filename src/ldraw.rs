use std::hash::{Hash, Hasher};
use palette::Srgba;
use crate::Color;

#[derive(Copy, Clone, Eq)]
pub struct LdrawColor {
    id: u16,
    value: Srgba<u8>
}

impl LdrawColor {
    pub const fn new(id: u16, red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        LdrawColor { id, value: Srgba::new(red, green, blue, alpha), }
    }
}

impl Default for LdrawColor {
    fn default() -> Self {
        BLACK
    }
}

impl PartialEq<Self> for LdrawColor {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Hash for LdrawColor {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut key = 0u64;
        key |= (self.value.red as u64) << 48;
        key |= (self.value.green as u64) << 32;
        key |= (self.value.blue as u64) << 16;
        key |= self.value.alpha as u64;
        state.write_u64(key)
    }
}

impl From<LdrawColor> for Srgba<u8> {
    fn from(color: LdrawColor) -> Self {
        color.value
    }
}

impl Color for LdrawColor {}

const BLACK: LdrawColor = LdrawColor::new(0, 27, 42, 52, 255);
const BLUE: LdrawColor = LdrawColor::new(1, 30, 90, 168, 255);
const GREEN: LdrawColor = LdrawColor::new(2, 0, 133, 43, 255);
const DARK_TURQUOISE: LdrawColor = LdrawColor::new(3, 6, 157, 159, 255);
const RED: LdrawColor = LdrawColor::new(4, 180, 0, 0, 255);
const DARK_PINK: LdrawColor = LdrawColor::new(5, 211, 53, 157, 255);
const BROWN: LdrawColor = LdrawColor::new(6, 84, 51, 36, 255);
const LIGHT_GREY: LdrawColor = LdrawColor::new(7, 138, 146, 141, 255);
const DARK_GREY: LdrawColor = LdrawColor::new(8, 84, 89, 85, 255);
const LIGHT_BLUE: LdrawColor = LdrawColor::new(9, 151, 203, 217, 255);
const BRIGHT_GREEN: LdrawColor = LdrawColor::new(10, 88, 171, 65, 255);
const LIGHT_TURQUOISE: LdrawColor = LdrawColor::new(11, 0, 170, 164, 255);
const SALMON: LdrawColor = LdrawColor::new(12, 240, 109, 97, 255);
const PINK: LdrawColor = LdrawColor::new(13, 246, 169, 187, 255);
const YELLOW: LdrawColor = LdrawColor::new(14, 250, 200, 10, 255);
const WHITE: LdrawColor = LdrawColor::new(15, 244, 244, 244, 255);
const LIGHT_GREEN: LdrawColor = LdrawColor::new(17, 173, 217, 168, 255);
const LIGHT_YELLOW: LdrawColor = LdrawColor::new(18, 255, 214, 127, 255);
const TAN: LdrawColor = LdrawColor::new(19, 176, 160, 111, 255);
const LIGHT_VIOLET: LdrawColor = LdrawColor::new(20, 175, 190, 214, 255);
const PURPLE: LdrawColor = LdrawColor::new(22, 103, 31, 129, 255);
const DARK_BLUE_VIOLET: LdrawColor = LdrawColor::new(23, 14, 62, 154, 255);
const ORANGE: LdrawColor = LdrawColor::new(25, 214, 121, 35, 255);
const MAGENTA: LdrawColor = LdrawColor::new(26, 144, 31, 118, 255);
const LIME: LdrawColor = LdrawColor::new(27, 165, 202, 24, 255);
const DARK_TAN: LdrawColor = LdrawColor::new(28, 137, 125, 98, 255);
const BRIGHT_PINK: LdrawColor = LdrawColor::new(29, 255, 158, 205, 255);
const MEDIUM_LAVENDER: LdrawColor = LdrawColor::new(30, 160, 110, 185, 255);
const LAVENDER: LdrawColor = LdrawColor::new(31, 205, 164, 222, 255);
const VERY_LIGHT_ORANGE: LdrawColor = LdrawColor::new(68, 253, 195, 131, 255);
const BRIGHT_REDDISH_LILAC: LdrawColor = LdrawColor::new(69, 138, 18, 168, 255);
const REDDISH_BROWN: LdrawColor = LdrawColor::new(70, 95, 49, 9, 255);
const LIGHT_BLUISH_GREY: LdrawColor = LdrawColor::new(71, 150, 150, 150, 255);
const DARK_BLUISH_GREY: LdrawColor = LdrawColor::new(72, 100, 100, 100, 255);
const MEDIUM_BLUE: LdrawColor = LdrawColor::new(73, 115, 150, 200, 255);
const MEDIUM_GREEN: LdrawColor = LdrawColor::new(74, 127, 196, 117, 255);
const LIGHT_PINK: LdrawColor = LdrawColor::new(77, 254, 204, 207, 255);
const LIGHT_NOUGAT: LdrawColor = LdrawColor::new(78, 255, 201, 149, 255);
const MEDIUM_NOUGAT: LdrawColor = LdrawColor::new(84, 170, 125, 85, 255);
const MEDIUM_LILAC: LdrawColor = LdrawColor::new(85, 68, 26, 145, 255);
const LIGHT_BROWN: LdrawColor = LdrawColor::new(86, 123, 93, 65, 255);
const BLUE_VIOLET: LdrawColor = LdrawColor::new(89, 28, 88, 167, 255);
const NOUGAT: LdrawColor = LdrawColor::new(92, 187, 128, 90, 255);
const LIGHT_SALMON: LdrawColor = LdrawColor::new(100, 249, 183, 165, 255);
const VIOLET: LdrawColor = LdrawColor::new(110, 38, 70, 154, 255);
const MEDIUM_VIOLET: LdrawColor = LdrawColor::new(112, 72, 97, 172, 255);
const MEDIUM_LIME: LdrawColor = LdrawColor::new(115, 183, 212, 37, 255);
const AQUA: LdrawColor = LdrawColor::new(118, 156, 214, 204, 255);
const LIGHT_LIME: LdrawColor = LdrawColor::new(120, 222, 234, 146, 255);
const LIGHT_ORANGE: LdrawColor = LdrawColor::new(125, 249, 167, 119, 255);
const DARK_NOUGAT: LdrawColor = LdrawColor::new(128, 173, 97, 64, 255);
const VERY_LIGHT_BLUISH_GREY: LdrawColor = LdrawColor::new(151, 200, 200, 200, 255);
const BRIGHT_LIGHT_ORANGE: LdrawColor = LdrawColor::new(191, 252, 172, 0, 255);
const BRIGHT_LIGHT_BLUE: LdrawColor = LdrawColor::new(212, 157, 195, 247, 255);
const RUST: LdrawColor = LdrawColor::new(216, 135, 43, 23, 255);
const REDDISH_LILAC: LdrawColor = LdrawColor::new(218, 142, 85, 151, 255);
const LILAC: LdrawColor = LdrawColor::new(219, 86, 78, 157, 255);
const BRIGHT_LIGHT_YELLOW: LdrawColor = LdrawColor::new(226, 255, 236, 108, 255);
const SKY_BLUE: LdrawColor = LdrawColor::new(232, 119, 201, 216, 255);
const DARK_BLUE: LdrawColor = LdrawColor::new(272, 25, 50, 90, 255);
const DARK_GREEN: LdrawColor = LdrawColor::new(288, 0, 69, 26, 255);
const FLAMINGO_PINK: LdrawColor = LdrawColor::new(295, 255, 148, 194, 255);
const DARK_BROWN: LdrawColor = LdrawColor::new(308, 53, 33, 0, 255);
const MAERSK_BLUE: LdrawColor = LdrawColor::new(313, 171, 217, 255, 255);
const DARK_RED: LdrawColor = LdrawColor::new(320, 114, 0, 18, 255);
const DARK_AZURE: LdrawColor = LdrawColor::new(321, 70, 155, 195, 255);
const MEDIUM_AZURE: LdrawColor = LdrawColor::new(322, 104, 195, 226, 255);
const LIGHT_AQUA: LdrawColor = LdrawColor::new(323, 211, 242, 234, 255);
const YELLOWISH_GREEN: LdrawColor = LdrawColor::new(326, 226, 249, 154, 255);
const OLIVE_GREEN: LdrawColor = LdrawColor::new(330, 119, 119, 78, 255);
const SAND_RED: LdrawColor = LdrawColor::new(335, 136, 96, 94, 255);
const MEDIUM_DARK_PINK: LdrawColor = LdrawColor::new(351, 247, 133, 177, 255);
const CORAL: LdrawColor = LdrawColor::new(353, 255, 109, 119, 255);
const EARTH_ORANGE: LdrawColor = LdrawColor::new(366, 216, 109, 44, 255);
const NEON_YELLOW: LdrawColor = LdrawColor::new(368, 237, 255, 33, 255);
const MEDIUM_BROWN: LdrawColor = LdrawColor::new(370, 117, 89, 69, 255);
const MEDIUM_TAN: LdrawColor = LdrawColor::new(371, 204, 163, 115, 255);
const SAND_PURPLE: LdrawColor = LdrawColor::new(373, 117, 101, 125, 255);
const SAND_GREEN: LdrawColor = LdrawColor::new(378, 112, 142, 124, 255);
const SAND_BLUE: LdrawColor = LdrawColor::new(379, 112, 129, 154, 255);
const FABULAND_BROWN: LdrawColor = LdrawColor::new(450, 210, 119, 68, 255);
const MEDIUM_ORANGE: LdrawColor = LdrawColor::new(462, 245, 134, 36, 255);
const DARK_ORANGE: LdrawColor = LdrawColor::new(484, 145, 80, 28, 255);
const VERY_LIGHT_GREY: LdrawColor = LdrawColor::new(503, 188, 180, 165, 255);
const LIGHT_ORANGE_BROWN: LdrawColor = LdrawColor::new(507, 250, 156, 28, 255);
const FABULAND_RED: LdrawColor = LdrawColor::new(508, 255, 128, 20, 255);
const FABULAND_ORANGE: LdrawColor = LdrawColor::new(509, 207, 138, 71, 255);
const FABULAND_LIME: LdrawColor = LdrawColor::new(510, 120, 252, 120, 255);
const TRANS_DARK_BLUE: LdrawColor = LdrawColor::new(33, 0, 32, 160, 128);
const TRANS_GREEN: LdrawColor = LdrawColor::new(34, 35, 120, 65, 128);
const TRANS_BRIGHT_GREEN: LdrawColor = LdrawColor::new(35, 86, 230, 70, 128);
const TRANS_RED: LdrawColor = LdrawColor::new(36, 201, 26, 9, 128);
const TRANS_DARK_PINK: LdrawColor = LdrawColor::new(37, 223, 102, 149, 128);
const TRANS_NEON_ORANGE: LdrawColor = LdrawColor::new(38, 255, 128, 13, 128);
const TRANS_VERY_LIGHT_BLUE: LdrawColor = LdrawColor::new(39, 193, 223, 240, 128);
const TRANS_BLACK: LdrawColor = LdrawColor::new(40, 99, 95, 82, 128);
const TRANS_MEDIUM_BLUE: LdrawColor = LdrawColor::new(41, 85, 154, 183, 128);
const TRANS_NEON_GREEN: LdrawColor = LdrawColor::new(42, 192, 255, 0, 128);
const TRANS_LIGHT_BLUE: LdrawColor = LdrawColor::new(43, 174, 233, 239, 128);
const TRANS_BRIGHT_REDDISH_LILAC: LdrawColor = LdrawColor::new(44, 150, 112, 159, 128);
const TRANS_PINK: LdrawColor = LdrawColor::new(45, 252, 151, 172, 128);
const TRANS_YELLOW: LdrawColor = LdrawColor::new(46, 245, 205, 47, 128);
const TRANS_CLEAR: LdrawColor = LdrawColor::new(47, 252, 252, 252, 128);
const TRANS_PURPLE: LdrawColor = LdrawColor::new(52, 165, 165, 203, 128);
const TRANS_NEON_YELLOW: LdrawColor = LdrawColor::new(54, 218, 176, 0, 128);
const TRANS_ORANGE: LdrawColor = LdrawColor::new(57, 240, 143, 28, 128);
const TRANS_BRIGHT_LIGHT_GREEN: LdrawColor = LdrawColor::new(227, 181, 217, 108, 128);
const TRANS_BRIGHT_LIGHT_ORANGE: LdrawColor = LdrawColor::new(231, 252, 183, 109, 128);
const TRANS_FIRE_YELLOW: LdrawColor = LdrawColor::new(234, 251, 232, 144, 128);
const TRANS_REDDISH_LILAC: LdrawColor = LdrawColor::new(284, 194, 129, 165, 128);
const TRANS_LIGHT_GREEN: LdrawColor = LdrawColor::new(285, 125, 194, 145, 128);
const TRANS_LIGHT_BLUE_VIOLET: LdrawColor = LdrawColor::new(293, 107, 171, 228, 128);
const CHROME_ANTIQUE_BRASS: LdrawColor = LdrawColor::new(60, 100, 90, 76, 255);
const CHROME_BLUE: LdrawColor = LdrawColor::new(61, 108, 150, 191, 255);
const CHROME_GREEN: LdrawColor = LdrawColor::new(62, 60, 179, 113, 255);
const CHROME_PINK: LdrawColor = LdrawColor::new(63, 170, 77, 142, 255);
const CHROME_BLACK: LdrawColor = LdrawColor::new(64, 27, 42, 52, 255);
const CHROME_GOLD: LdrawColor = LdrawColor::new(334, 223, 193, 118, 255);
const CHROME_SILVER: LdrawColor = LdrawColor::new(383, 206, 206, 206, 255);
const PEARL_BLACK: LdrawColor = LdrawColor::new(83, 10, 19, 39, 255);
const COPPER: LdrawColor = LdrawColor::new(134, 118, 77, 59, 255);
const PEARL_LIGHT_GREY: LdrawColor = LdrawColor::new(135, 160, 160, 160, 255);
const METALLIC_BLUE: LdrawColor = LdrawColor::new(137, 91, 117, 144, 255);
const PEARL_LIGHT_GOLD: LdrawColor = LdrawColor::new(142, 222, 172, 102, 255);
const PEARL_DARK_GOLD: LdrawColor = LdrawColor::new(147, 131, 114, 79, 255);
const PEARL_DARK_GREY: LdrawColor = LdrawColor::new(148, 72, 77, 72, 255);
const PEARL_VERY_LIGHT_GREY: LdrawColor = LdrawColor::new(150, 152, 155, 153, 255);
const PEARL_RED: LdrawColor = LdrawColor::new(176, 148, 81, 72, 255);
const PEARL_YELLOW: LdrawColor = LdrawColor::new(178, 171, 103, 58, 255);
const PEARL_SILVER: LdrawColor = LdrawColor::new(179, 137, 135, 136, 255);
const PEARL_WHITE: LdrawColor = LdrawColor::new(183, 246, 242, 223, 255);
const METALLIC_BRIGHT_RED: LdrawColor = LdrawColor::new(184, 214, 0, 38, 255);
const METALLIC_BRIGHT_BLUE: LdrawColor = LdrawColor::new(185, 0, 89, 163, 255);
const METALLIC_DARK_GREEN: LdrawColor = LdrawColor::new(186, 0, 142, 60, 255);
const REDDISH_GOLD: LdrawColor = LdrawColor::new(189, 172, 130, 71, 255);
const LEMON_METALLIC: LdrawColor = LdrawColor::new(200, 112, 130, 36, 255);
const PEARL_GOLD: LdrawColor = LdrawColor::new(297, 170, 127, 46, 255);
const METALLIC_SILVER: LdrawColor = LdrawColor::new(80, 118, 118, 118, 255);
const METALLIC_GREEN: LdrawColor = LdrawColor::new(81, 194, 192, 111, 255);
const METALLIC_GOLD: LdrawColor = LdrawColor::new(82, 219, 172, 52, 255);
const METALLIC_DARK_GREY: LdrawColor = LdrawColor::new(87, 62, 60, 57, 255);
const METALLIC_COPPER: LdrawColor = LdrawColor::new(300, 194, 127, 83, 255);
const METALLIC_LIGHT_BLUE: LdrawColor = LdrawColor::new(10045, 151, 203, 217, 255);
const METALLIC_PINK: LdrawColor = LdrawColor::new(10046, 173, 101, 154, 255);
const METALLIC_LIGHT_PINK: LdrawColor = LdrawColor::new(10049, 254, 204, 207, 255);
const MILKY_WHITE: LdrawColor = LdrawColor::new(79, 238, 238, 238, 240);
const GLOW_IN_DARK_OPAQUE: LdrawColor = LdrawColor::new(21, 224, 255, 176, 240);
const GLOW_IN_DARK_TRANS: LdrawColor = LdrawColor::new(294, 189, 198, 173, 240);
const GLOW_IN_DARK_WHITE: LdrawColor = LdrawColor::new(329, 245, 243, 215, 240);
const GLITTER_TRANS_DARK_PINK: LdrawColor = LdrawColor::new(114, 223, 102, 149, 128);
const GLITTER_TRANS_CLEAR: LdrawColor = LdrawColor::new(117, 238, 238, 238, 128);
const GLITTER_TRANS_PURPLE: LdrawColor = LdrawColor::new(129, 100, 0, 97, 128);
const GLITTER_TRANS_LIGHT_BLUE: LdrawColor = LdrawColor::new(302, 174, 233, 239, 128);
const GLITTER_TRANS_NEON_GREEN: LdrawColor = LdrawColor::new(339, 192, 255, 0, 128);
const GLITTER_TRANS_ORANGE: LdrawColor = LdrawColor::new(341, 240, 143, 28, 128);
const OPAL_TRANS_CLEAR: LdrawColor = LdrawColor::new(360, 252, 252, 252, 240);
const OPAL_TRANS_LIGHT_BLUE: LdrawColor = LdrawColor::new(362, 174, 233, 239, 200);
const OPAL_TRANS_BLACK: LdrawColor = LdrawColor::new(363, 99, 95, 82, 200);
const OPAL_TRANS_DARK_PINK: LdrawColor = LdrawColor::new(364, 223, 102, 149, 200);
const OPAL_TRANS_PURPLE: LdrawColor = LdrawColor::new(365, 103, 31, 129, 200);
const OPAL_TRANS_GREEN: LdrawColor = LdrawColor::new(367, 35, 120, 65, 200);
const GLITTER_TRANS_BRIGHT_GREEN: LdrawColor = LdrawColor::new(10351, 86, 230, 70, 128);
const OPAL_TRANS_DARK_BLUE: LdrawColor = LdrawColor::new(10366, 0, 32, 160, 200);

const SOLID_COLORS: &[LdrawColor] = &[
    BLACK,
    BLUE,
    GREEN,
    DARK_TURQUOISE,
    RED,
    DARK_PINK,
    BROWN,
    LIGHT_GREY,
    DARK_GREY,
    LIGHT_BLUE,
    BRIGHT_GREEN,
    LIGHT_TURQUOISE,
    SALMON,
    PINK,
    YELLOW,
    WHITE,
    LIGHT_GREEN,
    LIGHT_YELLOW,
    TAN,
    LIGHT_VIOLET,
    PURPLE,
    DARK_BLUE_VIOLET,
    ORANGE,
    MAGENTA,
    LIME,
    DARK_TAN,
    BRIGHT_PINK,
    MEDIUM_LAVENDER,
    LAVENDER,
    VERY_LIGHT_ORANGE,
    BRIGHT_REDDISH_LILAC,
    REDDISH_BROWN,
    LIGHT_BLUISH_GREY,
    DARK_BLUISH_GREY,
    MEDIUM_BLUE,
    MEDIUM_GREEN,
    LIGHT_PINK,
    LIGHT_NOUGAT,
    MEDIUM_NOUGAT,
    MEDIUM_LILAC,
    LIGHT_BROWN,
    BLUE_VIOLET,
    NOUGAT,
    LIGHT_SALMON,
    VIOLET,
    MEDIUM_VIOLET,
    MEDIUM_LIME,
    AQUA,
    LIGHT_LIME,
    LIGHT_ORANGE,
    DARK_NOUGAT,
    VERY_LIGHT_BLUISH_GREY,
    BRIGHT_LIGHT_ORANGE,
    BRIGHT_LIGHT_BLUE,
    RUST,
    REDDISH_LILAC,
    LILAC,
    BRIGHT_LIGHT_YELLOW,
    SKY_BLUE,
    DARK_BLUE,
    DARK_GREEN,
    FLAMINGO_PINK,
    DARK_BROWN,
    MAERSK_BLUE,
    DARK_RED,
    DARK_AZURE,
    MEDIUM_AZURE,
    LIGHT_AQUA,
    YELLOWISH_GREEN,
    OLIVE_GREEN,
    SAND_RED,
    MEDIUM_DARK_PINK,
    CORAL,
    EARTH_ORANGE,
    NEON_YELLOW,
    MEDIUM_BROWN,
    MEDIUM_TAN,
    SAND_PURPLE,
    SAND_GREEN,
    SAND_BLUE,
    FABULAND_BROWN,
    MEDIUM_ORANGE,
    DARK_ORANGE,
    VERY_LIGHT_GREY,
    LIGHT_ORANGE_BROWN,
    FABULAND_RED,
    FABULAND_ORANGE,
    FABULAND_LIME
];

const TRANSLUCENT_COLORS: &[LdrawColor] = &[
    TRANS_DARK_BLUE,
    TRANS_GREEN,
    TRANS_BRIGHT_GREEN,
    TRANS_RED,
    TRANS_DARK_PINK,
    TRANS_NEON_ORANGE,
    TRANS_VERY_LIGHT_BLUE,
    TRANS_BLACK,
    TRANS_MEDIUM_BLUE,
    TRANS_NEON_GREEN,
    TRANS_LIGHT_BLUE,
    TRANS_BRIGHT_REDDISH_LILAC,
    TRANS_PINK,
    TRANS_YELLOW,
    TRANS_CLEAR,
    TRANS_PURPLE,
    TRANS_NEON_YELLOW,
    TRANS_ORANGE,
    TRANS_BRIGHT_LIGHT_GREEN,
    TRANS_BRIGHT_LIGHT_ORANGE,
    TRANS_FIRE_YELLOW,
    TRANS_REDDISH_LILAC,
    TRANS_LIGHT_GREEN,
    TRANS_LIGHT_BLUE_VIOLET
];

const CHROME_COLORS: &[LdrawColor] = &[
    CHROME_ANTIQUE_BRASS,
    CHROME_BLUE,
    CHROME_GREEN,
    CHROME_PINK,
    CHROME_BLACK,
    CHROME_GOLD,
    CHROME_SILVER
];

const PEARLESCENT_COLORS: &[LdrawColor] = &[
    PEARL_BLACK,
    COPPER,
    PEARL_LIGHT_GREY,
    METALLIC_BLUE,
    PEARL_LIGHT_GOLD,
    PEARL_DARK_GOLD,
    PEARL_DARK_GREY,
    PEARL_VERY_LIGHT_GREY,
    PEARL_RED,
    PEARL_YELLOW,
    PEARL_SILVER,
    PEARL_WHITE,
    METALLIC_BRIGHT_RED,
    METALLIC_BRIGHT_BLUE,
    METALLIC_DARK_GREEN,
    REDDISH_GOLD,
    LEMON_METALLIC,
    PEARL_GOLD
];

const METALLIC_COLORS: &[LdrawColor] = &[
    METALLIC_SILVER,
    METALLIC_GREEN,
    METALLIC_GOLD,
    METALLIC_DARK_GREY,
    METALLIC_COPPER,
    METALLIC_LIGHT_BLUE,
    METALLIC_PINK,
    METALLIC_LIGHT_PINK
];

const MILKY_COLORS: &[LdrawColor] = &[
    MILKY_WHITE,
    GLOW_IN_DARK_OPAQUE,
    GLOW_IN_DARK_TRANS,
    GLOW_IN_DARK_WHITE
];

const GLITTER_COLORS: &[LdrawColor] = &[
    GLITTER_TRANS_DARK_PINK,
    GLITTER_TRANS_CLEAR,
    GLITTER_TRANS_PURPLE,
    GLITTER_TRANS_LIGHT_BLUE,
    GLITTER_TRANS_NEON_GREEN,
    GLITTER_TRANS_ORANGE,
    OPAL_TRANS_CLEAR,
    OPAL_TRANS_LIGHT_BLUE,
    OPAL_TRANS_BLACK,
    OPAL_TRANS_DARK_PINK,
    OPAL_TRANS_PURPLE,
    OPAL_TRANS_GREEN,
    GLITTER_TRANS_BRIGHT_GREEN,
    OPAL_TRANS_DARK_BLUE
];