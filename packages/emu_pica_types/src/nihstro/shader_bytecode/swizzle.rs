// Copyright 2014 Tony Wasserka
// All rights reserved.
//
// Redistribution and use in source and binary forms, with or without
// modification, are permitted provided that the following conditions are met:
//
//     * Redistributions of source code must retain the above copyright
//       notice, this list of conditions and the following disclaimer.
//     * Redistributions in binary form must reproduce the above copyright
//       notice, this list of conditions and the following disclaimer in the
//       documentation and/or other materials provided with the distribution.
//     * Neither the name of the owner nor the names of its contributors may
//       be used to endorse or promote products derived from this software
//       without specific prior written permission.
//
// THIS SOFTWARE IS PROVIDED BY THE COPYRIGHT HOLDERS AND CONTRIBUTORS
// "AS IS" AND ANY EXPRESS OR IMPLIED WARRANTIES, INCLUDING, BUT NOT
// LIMITED TO, THE IMPLIED WARRANTIES OF MERCHANTABILITY AND FITNESS FOR
// A PARTICULAR PURPOSE ARE DISCLAIMED. IN NO EVENT SHALL THE COPYRIGHT
// OWNER OR CONTRIBUTORS BE LIABLE FOR ANY DIRECT, INDIRECT, INCIDENTAL,
// SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES (INCLUDING, BUT NOT
// LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR SERVICES; LOSS OF USE,
// DATA, OR PROFITS; OR BUSINESS INTERRUPTION) HOWEVER CAUSED AND ON ANY
// THEORY OF LIABILITY, WHETHER IN CONTRACT, STRICT LIABILITY, OR TORT
// (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING IN ANY WAY OUT OF THE USE
// OF THIS SOFTWARE, EVEN IF ADVISED OF THE POSSIBILITY OF SUCH DAMAGE.

use crate::bits::{bits, mask, pica_enum};

pica_enum!(Selector {
    X = 0,
    Y = 1,
    Z = 2,
    W = 3,
});

#[repr(transparent)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SwizzlePattern {
    pub hex: u32,
}

const _: () = assert!(
    size_of::<SwizzlePattern>() == 0x4,
    "Incorrect structure size"
);

impl SwizzlePattern {
    fn set(&mut self, position: u32, width: u32, value: u32) {
        let field = mask(width) << position;
        self.hex = (self.hex & !field) | ((value << position) & field);
    }

    /**
     * Gets the raw 8-bit selector for the specified (1-indexed) source register.
     */
    pub fn get_raw_selector(self, src: u32) -> u32 {
        if src == 0 || src > 3 {
            panic!("src needs to be between 1 and 3");
        }

        let selectors = [
            self.src1_selector(),
            self.src2_selector(),
            self.src3_selector(),
        ];
        selectors[src as usize - 1]
    }

    pub fn get_selector_src1(self, comp: usize) -> Selector {
        let selectors = [
            self.src1_selector_0(),
            self.src1_selector_1(),
            self.src1_selector_2(),
            self.src1_selector_3(),
        ];
        selectors[comp]
    }

    pub fn get_selector_src2(self, comp: usize) -> Selector {
        let selectors = [
            self.src2_selector_0(),
            self.src2_selector_1(),
            self.src2_selector_2(),
            self.src2_selector_3(),
        ];
        selectors[comp]
    }

    pub fn get_selector_src3(self, comp: usize) -> Selector {
        let selectors = [
            self.src3_selector_0(),
            self.src3_selector_1(),
            self.src3_selector_2(),
            self.src3_selector_3(),
        ];
        selectors[comp]
    }

    pub fn set_selector_src1(&mut self, comp: usize, value: Selector) {
        if comp > 3 {
            panic!("comp needs to be smaller than 4");
        }
        self.set(11 - 2 * comp as u32, 2, value.0);
    }

    pub fn set_selector_src2(&mut self, comp: usize, value: Selector) {
        if comp > 3 {
            panic!("comp needs to be smaller than 4");
        }
        self.set(20 - 2 * comp as u32, 2, value.0);
    }

    pub fn set_selector_src3(&mut self, comp: usize, value: Selector) {
        if comp > 3 {
            panic!("comp needs to be smaller than 4");
        }
        self.set(29 - 2 * comp as u32, 2, value.0);
    }

    pub fn selector_to_string(self, src2: bool) -> String {
        (0..4)
            .map(|i| {
                let selector = if src2 {
                    self.get_selector_src2(i)
                } else {
                    self.get_selector_src1(i)
                };
                ["x", "y", "z", "w"][selector.0 as usize]
            })
            .collect()
    }

    pub fn dest_component_enabled(self, i: u32) -> bool {
        (self.dest_mask() & (0x8 >> i)) != 0
    }

    pub fn set_dest_component_enabled(&mut self, i: u32, enabled: bool) {
        let mask = 0xffff & (0x8 >> i);
        let dest_mask = (self.dest_mask() & !mask) | (u32::from(enabled) * mask);
        self.set(0, 4, dest_mask);
    }

    pub fn dest_mask_to_string(self) -> String {
        (0..4)
            .map(|i| {
                if !self.dest_component_enabled(i) {
                    '_'
                } else {
                    ['x', 'y', 'z', 'w'][i as usize]
                }
            })
            .collect()
    }

    // Components of "dest" that should be written to: LSB=dest.w, MSB=dest.x
    pub fn dest_mask(self) -> u32 {
        bits(self.hex, 0, 4)
    }

    pub fn negate_src1(self) -> bool {
        bits(self.hex, 4, 1) != 0
    }

    pub fn src1_selector(self) -> u32 {
        bits(self.hex, 5, 8)
    }

    pub fn src1_selector_3(self) -> Selector {
        Selector(bits(self.hex, 5, 2))
    }

    pub fn src1_selector_2(self) -> Selector {
        Selector(bits(self.hex, 7, 2))
    }

    pub fn src1_selector_1(self) -> Selector {
        Selector(bits(self.hex, 9, 2))
    }

    pub fn src1_selector_0(self) -> Selector {
        Selector(bits(self.hex, 11, 2))
    }

    pub fn negate_src2(self) -> bool {
        bits(self.hex, 13, 1) != 0
    }

    pub fn src2_selector(self) -> u32 {
        bits(self.hex, 14, 8)
    }

    pub fn src2_selector_3(self) -> Selector {
        Selector(bits(self.hex, 14, 2))
    }

    pub fn src2_selector_2(self) -> Selector {
        Selector(bits(self.hex, 16, 2))
    }

    pub fn src2_selector_1(self) -> Selector {
        Selector(bits(self.hex, 18, 2))
    }

    pub fn src2_selector_0(self) -> Selector {
        Selector(bits(self.hex, 20, 2))
    }

    pub fn negate_src3(self) -> bool {
        bits(self.hex, 22, 1) != 0
    }

    pub fn src3_selector(self) -> u32 {
        bits(self.hex, 23, 8)
    }

    pub fn src3_selector_3(self) -> Selector {
        Selector(bits(self.hex, 23, 2))
    }

    pub fn src3_selector_2(self) -> Selector {
        Selector(bits(self.hex, 25, 2))
    }

    pub fn src3_selector_1(self) -> Selector {
        Selector(bits(self.hex, 27, 2))
    }

    pub fn src3_selector_0(self) -> Selector {
        Selector(bits(self.hex, 29, 2))
    }
}
