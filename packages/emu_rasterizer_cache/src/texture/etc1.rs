use emu_pica_types::Vec3;
use emu_pica_types::color::{convert_4_to_8, convert_5_to_8};

const ETC1_MODIFIER_TABLE: [[u8; 2]; 8] = [
    [2, 8],
    [5, 17],
    [9, 29],
    [13, 42],
    [18, 60],
    [24, 80],
    [33, 106],
    [47, 183],
];

const fn bits(value: u64, position: u32, width: u32) -> u64 {
    (value >> position) & ((1u64 << width) - 1)
}

const fn sbits(value: u64, position: u32, width: u32) -> i64 {
    ((value << (64 - position - width)) as i64) >> (64 - width)
}

#[derive(Clone, Copy)]
struct Etc1Tile {
    raw: u64,
}

impl Etc1Tile {
    // Each of these two is a collection of 16 bits (one per lookup value)
    fn table_subindexes(&self) -> u64 {
        bits(self.raw, 0, 16)
    }

    fn negation_flags(&self) -> u64 {
        bits(self.raw, 16, 16)
    }

    fn get_table_sub_index(&self, index: u32) -> u32 {
        ((self.table_subindexes() >> index) & 1) as u32
    }

    fn get_negation_flag(&self, index: u32) -> bool {
        ((self.negation_flags() >> index) & 1) == 1
    }

    fn flip(&self) -> bool {
        bits(self.raw, 32, 1) != 0
    }

    fn differential_mode(&self) -> bool {
        bits(self.raw, 33, 1) != 0
    }

    fn table_index_2(&self) -> u64 {
        bits(self.raw, 34, 3)
    }

    fn table_index_1(&self) -> u64 {
        bits(self.raw, 37, 3)
    }

    // delta value + base value
    fn differential_db(&self) -> i64 {
        sbits(self.raw, 40, 3)
    }

    fn differential_b(&self) -> u64 {
        bits(self.raw, 43, 5)
    }

    fn differential_dg(&self) -> i64 {
        sbits(self.raw, 48, 3)
    }

    fn differential_g(&self) -> u64 {
        bits(self.raw, 51, 5)
    }

    fn differential_dr(&self) -> i64 {
        sbits(self.raw, 56, 3)
    }

    fn differential_r(&self) -> u64 {
        bits(self.raw, 59, 5)
    }

    fn separate_b2(&self) -> u64 {
        bits(self.raw, 40, 4)
    }

    fn separate_b1(&self) -> u64 {
        bits(self.raw, 44, 4)
    }

    fn separate_g2(&self) -> u64 {
        bits(self.raw, 48, 4)
    }

    fn separate_g1(&self) -> u64 {
        bits(self.raw, 52, 4)
    }

    fn separate_r2(&self) -> u64 {
        bits(self.raw, 56, 4)
    }

    fn separate_r1(&self) -> u64 {
        bits(self.raw, 60, 4)
    }

    fn get_rgb(&self, mut x: u32, mut y: u32) -> Vec3<u8> {
        let texel = 4 * x + y;

        if self.flip() {
            std::mem::swap(&mut x, &mut y);
        }

        // Lookup base value
        let mut ret: [i32; 3];
        if self.differential_mode() {
            ret = [
                self.differential_r() as i32,
                self.differential_g() as i32,
                self.differential_b() as i32,
            ];
            if x >= 2 {
                ret[0] += self.differential_dr() as i32;
                ret[1] += self.differential_dg() as i32;
                ret[2] += self.differential_db() as i32;
            }
            ret = ret.map(|component| i32::from(convert_5_to_8(component as u8)));
        } else if x < 2 {
            ret = [
                i32::from(convert_4_to_8(self.separate_r1() as u8)),
                i32::from(convert_4_to_8(self.separate_g1() as u8)),
                i32::from(convert_4_to_8(self.separate_b1() as u8)),
            ];
        } else {
            ret = [
                i32::from(convert_4_to_8(self.separate_r2() as u8)),
                i32::from(convert_4_to_8(self.separate_g2() as u8)),
                i32::from(convert_4_to_8(self.separate_b2() as u8)),
            ];
        }

        // Add modifier
        let table_index = if x < 2 {
            self.table_index_1()
        } else {
            self.table_index_2()
        } as usize;

        let mut modifier =
            i32::from(ETC1_MODIFIER_TABLE[table_index][self.get_table_sub_index(texel) as usize]);
        if self.get_negation_flag(texel) {
            modifier *= -1;
        }

        let ret = ret.map(|component| (component + modifier).clamp(0, 255) as u8);
        Vec3 {
            x: ret[0],
            y: ret[1],
            z: ret[2],
        }
    }
}

pub fn sample_etc1_subtile(value: u64, x: u32, y: u32) -> Vec3<u8> {
    let tile = Etc1Tile { raw: value };
    tile.get_rgb(x, y)
}
