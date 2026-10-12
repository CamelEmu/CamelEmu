//! An encoder of the AArch64 instructions that the translator emits.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Reg(pub u32);

pub const X0: Reg = Reg(0);
pub const X1: Reg = Reg(1);
pub const X2: Reg = Reg(2);
pub const X8: Reg = Reg(8);
pub const X9: Reg = Reg(9);
pub const X10: Reg = Reg(10);
pub const X11: Reg = Reg(11);
pub const X12: Reg = Reg(12);
pub const X13: Reg = Reg(13);
pub const X14: Reg = Reg(14);
pub const X16: Reg = Reg(16);
pub const X19: Reg = Reg(19);
pub const X20: Reg = Reg(20);
pub const X21: Reg = Reg(21);
pub const X22: Reg = Reg(22);
pub const X23: Reg = Reg(23);
pub const X24: Reg = Reg(24);
pub const X25: Reg = Reg(25);
pub const X26: Reg = Reg(26);
pub const X29: Reg = Reg(29);
pub const X30: Reg = Reg(30);
/// The zero register, or SP where an instruction takes it.
pub const ZR: Reg = Reg(31);
pub const SP: Reg = Reg(31);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShiftKind {
    Lsl = 0,
}

/// The condition codes, which have the same values as the ones of ARM.
pub type Cond = u32;
pub const COND_EQ: Cond = 0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Label(usize);

enum Fixup {
    /// B and BL, with a 26-bit offset.
    Branch26,
    /// B.cond, CBZ, CBNZ and LDR (literal), with a 19-bit offset.
    Branch19,
    /// TBZ and TBNZ, with a 14-bit offset.
    Branch14,
}

pub struct Assembler {
    pub code: Vec<u32>,
    labels: Vec<Option<usize>>,
    fixups: Vec<(usize, Label, Fixup)>,
    literals: Vec<(Label, u64)>,
}

impl Assembler {
    pub fn new() -> Self {
        Self {
            code: Vec::new(),
            labels: Vec::new(),
            fixups: Vec::new(),
            literals: Vec::new(),
        }
    }

    fn emit(&mut self, word: u32) {
        self.code.push(word);
    }

    pub fn new_label(&mut self) -> Label {
        self.labels.push(None);
        Label(self.labels.len() - 1)
    }

    pub fn bind(&mut self, label: Label) {
        self.labels[label.0] = Some(self.code.len());
    }

    fn reference(&mut self, label: Label, fixup: Fixup, word: u32) {
        self.fixups.push((self.code.len(), label, fixup));
        self.emit(word);
    }

    /// Emits the literal pool and resolves the labels. Returns `None` if an offset is out of range.
    pub fn finish(mut self) -> Option<Vec<u32>> {
        if !self.literals.is_empty() {
            if self.code.len() % 2 != 0 {
                self.emit(0xD503201F);
            }
            for (label, value) in std::mem::take(&mut self.literals) {
                self.bind(label);
                self.emit(value as u32);
                self.emit((value >> 32) as u32);
            }
        }
        for (at, label, fixup) in std::mem::take(&mut self.fixups) {
            let target = self.labels[label.0]?;
            let offset = target as i64 - at as i64;
            let (bits, shift) = match fixup {
                Fixup::Branch26 => (26, 0),
                Fixup::Branch19 => (19, 5),
                Fixup::Branch14 => (14, 5),
            };
            let limit = 1i64 << (bits - 1);
            if offset < -limit || offset >= limit {
                return None;
            }
            let field = (offset as u32) & ((1u32 << bits) - 1);
            self.code[at] |= field << shift;
        }
        Some(self.code)
    }

    // Moves and constants

    pub fn mov_w(&mut self, rd: Reg, rm: Reg) {
        self.emit(0x2A0003E0 | (rm.0 << 16) | rd.0);
    }

    pub fn mov_x(&mut self, rd: Reg, rm: Reg) {
        self.emit(0xAA0003E0 | (rm.0 << 16) | rd.0);
    }

    pub fn mov_imm32(&mut self, rd: Reg, value: u32) {
        self.emit(0x52800000 | ((value & 0xFFFF) << 5) | rd.0);
        if value >> 16 != 0 {
            self.emit(0x72A00000 | ((value >> 16) << 5) | rd.0);
        }
    }

    /// Loads a 64-bit constant from the literal pool.
    pub fn ldr_literal(&mut self, rt: Reg, value: u64) {
        let label = self.new_label();
        self.literals.push((label, value));
        self.reference(label, Fixup::Branch19, 0x58000000 | rt.0);
    }

    // Arithmetic and logic on 32-bit registers

    fn shifted(&mut self, base: u32, rd: Reg, rn: Reg, rm: Reg, shift: ShiftKind, amount: u32) {
        self.emit(base | ((shift as u32) << 22) | (rm.0 << 16) | (amount << 10) | (rn.0 << 5) | rd.0);
    }

    pub fn add_w(&mut self, rd: Reg, rn: Reg, rm: Reg) {
        self.shifted(0x0B000000, rd, rn, rm, ShiftKind::Lsl, 0);
    }
    pub fn adds_w(&mut self, rd: Reg, rn: Reg, rm: Reg) {
        self.shifted(0x2B000000, rd, rn, rm, ShiftKind::Lsl, 0);
    }
    pub fn sub_w(&mut self, rd: Reg, rn: Reg, rm: Reg) {
        self.shifted(0x4B000000, rd, rn, rm, ShiftKind::Lsl, 0);
    }
    pub fn subs_w(&mut self, rd: Reg, rn: Reg, rm: Reg) {
        self.shifted(0x6B000000, rd, rn, rm, ShiftKind::Lsl, 0);
    }
    pub fn adc_w(&mut self, rd: Reg, rn: Reg, rm: Reg) {
        self.emit(0x1A000000 | (rm.0 << 16) | (rn.0 << 5) | rd.0);
    }
    pub fn adcs_w(&mut self, rd: Reg, rn: Reg, rm: Reg) {
        self.emit(0x3A000000 | (rm.0 << 16) | (rn.0 << 5) | rd.0);
    }
    pub fn sbc_w(&mut self, rd: Reg, rn: Reg, rm: Reg) {
        self.emit(0x5A000000 | (rm.0 << 16) | (rn.0 << 5) | rd.0);
    }
    pub fn sbcs_w(&mut self, rd: Reg, rn: Reg, rm: Reg) {
        self.emit(0x7A000000 | (rm.0 << 16) | (rn.0 << 5) | rd.0);
    }
    pub fn and_w(&mut self, rd: Reg, rn: Reg, rm: Reg) {
        self.shifted(0x0A000000, rd, rn, rm, ShiftKind::Lsl, 0);
    }
    pub fn bic_w(&mut self, rd: Reg, rn: Reg, rm: Reg) {
        self.shifted(0x0A200000, rd, rn, rm, ShiftKind::Lsl, 0);
    }
    pub fn orr_w(&mut self, rd: Reg, rn: Reg, rm: Reg) {
        self.shifted(0x2A000000, rd, rn, rm, ShiftKind::Lsl, 0);
    }
    /// ORR with a shifted operand, which also combines flags.
    pub fn orr_w_shifted(&mut self, rd: Reg, rn: Reg, rm: Reg, shift: ShiftKind, amount: u32) {
        self.shifted(0x2A000000, rd, rn, rm, shift, amount);
    }
    pub fn orn_w(&mut self, rd: Reg, rn: Reg, rm: Reg) {
        self.shifted(0x2A200000, rd, rn, rm, ShiftKind::Lsl, 0);
    }
    pub fn eor_w(&mut self, rd: Reg, rn: Reg, rm: Reg) {
        self.shifted(0x4A000000, rd, rn, rm, ShiftKind::Lsl, 0);
    }

    pub fn add_imm_w(&mut self, rd: Reg, rn: Reg, imm12: u32) {
        self.emit(0x11000000 | (imm12 << 10) | (rn.0 << 5) | rd.0);
    }
    pub fn sub_imm_w(&mut self, rd: Reg, rn: Reg, imm12: u32) {
        self.emit(0x51000000 | (imm12 << 10) | (rn.0 << 5) | rd.0);
    }
    pub fn sub_imm_x(&mut self, rd: Reg, rn: Reg, imm12: u32) {
        self.emit(0xD1000000 | (imm12 << 10) | (rn.0 << 5) | rd.0);
    }
    pub fn sub_x(&mut self, rd: Reg, rn: Reg, rm: Reg) {
        self.emit(0xCB000000 | (rm.0 << 16) | (rn.0 << 5) | rd.0);
    }
    pub fn cmp_imm_w(&mut self, rn: Reg, imm12: u32) {
        self.emit(0x71000000 | (imm12 << 10) | (rn.0 << 5) | ZR.0);
    }

    // Bitfields and shifts

    pub fn ubfx_w(&mut self, rd: Reg, rn: Reg, lsb: u32, width: u32) {
        self.emit(0x53000000 | (lsb << 16) | ((lsb + width - 1) << 10) | (rn.0 << 5) | rd.0);
    }
    pub fn bfi_w(&mut self, rd: Reg, rn: Reg, lsb: u32, width: u32) {
        self.emit(0x33000000 | (((32 - lsb) % 32) << 16) | ((width - 1) << 10) | (rn.0 << 5) | rd.0);
    }
    pub fn lsl_imm_w(&mut self, rd: Reg, rn: Reg, shift: u32) {
        self.emit(0x53000000 | (((32 - shift) % 32) << 16) | ((31 - shift) << 10) | (rn.0 << 5) | rd.0);
    }
    pub fn lsr_imm_w(&mut self, rd: Reg, rn: Reg, shift: u32) {
        self.emit(0x53000000 | (shift << 16) | (31 << 10) | (rn.0 << 5) | rd.0);
    }
    pub fn asr_imm_w(&mut self, rd: Reg, rn: Reg, shift: u32) {
        self.emit(0x13000000 | (shift << 16) | (31 << 10) | (rn.0 << 5) | rd.0);
    }
    /// EXTR, which takes the 32 bits of `rn:rm` that start at the bit `lsb`.
    pub fn extr_w(&mut self, rd: Reg, rn: Reg, rm: Reg, lsb: u32) {
        self.emit(0x13800000 | (rm.0 << 16) | (lsb << 10) | (rn.0 << 5) | rd.0);
    }
    pub fn ror_imm_w(&mut self, rd: Reg, rn: Reg, shift: u32) {
        self.extr_w(rd, rn, rn, shift);
    }

    /// CSET, which writes 1 when the condition holds.
    pub fn cset_w(&mut self, rd: Reg, cond: Cond) {
        self.emit(0x1A9F07E0 | ((cond ^ 1) << 12) | rd.0);
    }

    // Flags

    pub fn mrs_nzcv(&mut self, rt: Reg) {
        self.emit(0xD53B4200 | rt.0);
    }

    pub fn msr_nzcv(&mut self, rt: Reg) {
        self.emit(0xD51B4200 | rt.0);
    }

    // Memory

    pub fn ldr_w(&mut self, rt: Reg, rn: Reg, offset: u32) {
        debug_assert!(offset % 4 == 0 && offset / 4 < 4096);
        self.emit(0xB9400000 | ((offset / 4) << 10) | (rn.0 << 5) | rt.0);
    }
    pub fn str_w(&mut self, rt: Reg, rn: Reg, offset: u32) {
        debug_assert!(offset % 4 == 0 && offset / 4 < 4096);
        self.emit(0xB9000000 | ((offset / 4) << 10) | (rn.0 << 5) | rt.0);
    }
    pub fn ldr_x(&mut self, rt: Reg, rn: Reg, offset: u32) {
        debug_assert!(offset % 8 == 0 && offset / 8 < 4096);
        self.emit(0xF9400000 | ((offset / 8) << 10) | (rn.0 << 5) | rt.0);
    }
    pub fn str_x(&mut self, rt: Reg, rn: Reg, offset: u32) {
        debug_assert!(offset % 8 == 0 && offset / 8 < 4096);
        self.emit(0xF9000000 | ((offset / 8) << 10) | (rn.0 << 5) | rt.0);
    }
    /// LDR Xt, [Xn, Xm, LSL #3].
    pub fn ldr_x_index(&mut self, rt: Reg, rn: Reg, rm: Reg) {
        self.emit(0xF8607800 | (rm.0 << 16) | (rn.0 << 5) | rt.0);
    }
    /// LDR Wt, [Xn, Xm].
    pub fn ldr_w_reg(&mut self, rt: Reg, rn: Reg, rm: Reg) {
        self.emit(0xB8606800 | (rm.0 << 16) | (rn.0 << 5) | rt.0);
    }
    pub fn str_w_reg(&mut self, rt: Reg, rn: Reg, rm: Reg) {
        self.emit(0xB8206800 | (rm.0 << 16) | (rn.0 << 5) | rt.0);
    }
    pub fn ldrb_reg(&mut self, rt: Reg, rn: Reg, rm: Reg) {
        self.emit(0x38606800 | (rm.0 << 16) | (rn.0 << 5) | rt.0);
    }
    pub fn strb_reg(&mut self, rt: Reg, rn: Reg, rm: Reg) {
        self.emit(0x38206800 | (rm.0 << 16) | (rn.0 << 5) | rt.0);
    }
    /// STP with a pre-indexed base, for the prologue.
    pub fn stp_x_pre(&mut self, rt: Reg, rt2: Reg, rn: Reg, offset: i32) {
        let imm7 = ((offset / 8) as u32) & 0x7F;
        self.emit(0xA9800000 | (imm7 << 15) | (rt2.0 << 10) | (rn.0 << 5) | rt.0);
    }
    /// LDP with a post-indexed base, for the epilogue.
    pub fn ldp_x_post(&mut self, rt: Reg, rt2: Reg, rn: Reg, offset: i32) {
        let imm7 = ((offset / 8) as u32) & 0x7F;
        self.emit(0xA8C00000 | (imm7 << 15) | (rt2.0 << 10) | (rn.0 << 5) | rt.0);
    }

    // Branches

    pub fn b(&mut self, label: Label) {
        self.reference(label, Fixup::Branch26, 0x14000000);
    }
    pub fn b_cond(&mut self, cond: Cond, label: Label) {
        self.reference(label, Fixup::Branch19, 0x54000000 | cond);
    }
    pub fn cbz_x(&mut self, rt: Reg, label: Label) {
        self.reference(label, Fixup::Branch19, 0xB4000000 | rt.0);
    }
    pub fn cbnz_w(&mut self, rt: Reg, label: Label) {
        self.reference(label, Fixup::Branch19, 0x35000000 | rt.0);
    }
    pub fn tbnz_w(&mut self, rt: Reg, bit: u32, label: Label) {
        self.reference(label, Fixup::Branch14, 0x37000000 | (bit << 19) | rt.0);
    }
    pub fn blr(&mut self, rn: Reg) {
        self.emit(0xD63F0000 | (rn.0 << 5));
    }
    pub fn ret(&mut self) {
        self.emit(0xD65F03C0);
    }
}
