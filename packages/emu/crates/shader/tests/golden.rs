use emu_shader::{Profile, generate_fragment_shader};
use sha2::{Digest, Sha256};

const DIGESTS: &str = include_str!("golden_digests.txt");
const CASES: u32 = 301;
const CONFIG_BYTES: usize = 248;

struct Rng(u32);

impl Rng {
    fn new(case: u32) -> Self {
        let mut rng = Self(case.wrapping_mul(0x9E37_79B1).wrapping_add(0x7F4A_7C15) | 1);
        rng.next();
        rng
    }

    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 17;
        self.0 ^= self.0 << 5;
        self.0
    }
}

fn get32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().unwrap())
}

fn set32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn build_config(case: u32) -> (Vec<u8>, u32) {
    let mut rng = Rng::new(case);
    let mut bytes = vec![0u8; CONFIG_BYTES];
    for offset in (0..CONFIG_BYTES).step_by(4) {
        set32(&mut bytes, offset, if case == 0 { 0 } else { rng.next() });
    }
    let profile_bits = if case == 0 { 0 } else { rng.next() };

    const LOGIC_OPS: [u32; 5] = [0, 3, 4, 5, 6];
    let mut raw = get32(&bytes, 0);
    let selected = ((raw >> 6) & 0xF) % 5;
    raw = (raw & !(0xF << 6)) | (LOGIC_OPS[selected as usize] << 6);
    set32(&mut bytes, 0, raw);

    const TEXTURE_TYPES: [u32; 5] = [0, 1, 2, 3, 5];
    raw = get32(&bytes, 28);
    let selected = (raw & 7) % 5;
    raw = (raw & !7) | TEXTURE_TYPES[selected as usize];
    set32(&mut bytes, 28, raw);

    raw = get32(&bytes, 140);
    let src_num = ((raw >> 1) & 0xF) % 9;
    raw = (raw & !(0xF << 1)) | (src_num << 1);
    set32(&mut bytes, 140, raw);

    const SCALES: [f32; 8] = [0.5, 1.0, 2.0, 4.0, 8.0, 0.25, 0.125, 16.0];
    for lut in 0..7 {
        let offset = 144 + lut * 8;
        raw = get32(&bytes, offset);
        let input_type = ((raw >> 2) & 7) % 6;
        raw = (raw & !(7 << 2)) | (input_type << 2);
        set32(&mut bytes, offset, raw);
        let scale_word = get32(&bytes, offset + 4);
        set32(
            &mut bytes,
            offset + 4,
            SCALES[((scale_word >> 3) % 8) as usize].to_bits(),
        );
    }

    raw = get32(&bytes, 216);
    let filter = ((raw >> 17) & 7) % 6;
    raw = (raw & !(7 << 17)) | (filter << 17);
    set32(&mut bytes, 216, raw);
    let lut_width = get32(&bytes, 220) % 255 + 1;
    set32(&mut bytes, 220, lut_width);
    for offset in (224..=236).step_by(4) {
        let value = get32(&bytes, offset) % 1024;
        set32(&mut bytes, offset, value);
    }
    let lod_word = get32(&bytes, 240);
    set32(
        &mut bytes,
        240,
        ((lod_word & 0xFFFF) % 12) | (((lod_word >> 16) % 12) << 16),
    );

    (bytes, profile_bits)
}

fn profile_from_bits(bits: u32) -> Profile {
    let flag = |index: u32| (bits >> index) & 1 != 0;
    Profile {
        has_separable_shaders: flag(0),
        has_clip_planes: flag(1),
        has_geometry_shader: flag(2),
        has_custom_border_color: flag(3),
        has_fragment_shader_interlock: flag(4),
        has_fragment_shader_barycentric: flag(5),
        has_blend_minmax_factor: flag(6),
        has_minus_one_to_one_range: flag(7),
        has_logic_op: flag(8),
        has_gl_ext_framebuffer_fetch: flag(9),
        has_gl_arm_framebuffer_fetch: flag(10),
        has_gl_nv_fragment_shader_interlock: flag(11),
        has_gl_intel_fragment_shader_ordering: flag(12),
        has_gl_nv_fragment_shader_barycentric: flag(13),
        is_vulkan: flag(14),
    }
}

fn normalize(words: &[u32]) -> Vec<u32> {
    const OP_CAPABILITY: u32 = 17;
    const OP_EXTENSION: u32 = 10;
    let mut result = words[..5].to_vec();
    let mut header: Vec<Vec<u32>> = Vec::new();
    let mut cursor = 5;
    while cursor < words.len() {
        let opcode = words[cursor] & 0xFFFF;
        let count = (words[cursor] >> 16) as usize;
        if opcode != OP_CAPABILITY && opcode != OP_EXTENSION {
            break;
        }
        header.push(words[cursor..cursor + count].to_vec());
        cursor += count;
    }
    header.sort();
    for instruction in header {
        result.extend(instruction);
    }
    result.extend_from_slice(&words[cursor..]);
    result
}

fn digest(words: &[u32]) -> String {
    let bytes: Vec<u8> = words.iter().flat_map(|word| word.to_le_bytes()).collect();
    Sha256::digest(&bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn fragment_shaders_match_the_original_generator() {
    let expected: Vec<&str> = DIGESTS
        .lines()
        .map(|line| line.split_whitespace().next().unwrap())
        .collect();
    assert_eq!(expected.len(), CASES as usize);

    for case in 0..CASES {
        let (config, profile_bits) = build_config(case);
        let words = generate_fragment_shader(&config, profile_from_bits(profile_bits)).unwrap();
        assert_eq!(
            digest(&normalize(&words)),
            expected[case as usize],
            "case {case}"
        );
    }
}
