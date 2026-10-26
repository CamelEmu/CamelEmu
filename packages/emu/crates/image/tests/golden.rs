use emu_image::{
    Android420, Android420Strides, Argb, BlendMode, Converter, FilterMode, I420, I420Mut, PlaneMut,
    RotationMode, ScaleSettings, Scaler, Strides, abgr_to_argb, android420_to_i420, argb_mirror,
    i420_mirror, i420_rotate,
};
use sha2::{Digest, Sha256};

struct Rng(u32);

impl Rng {
    fn new() -> Self {
        Self(12345)
    }

    fn fill(&mut self, bytes: &mut [u8]) {
        for byte in bytes {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 17;
            self.0 ^= self.0 << 5;
            *byte = (self.0 >> 24) as u8;
        }
    }
}

fn half(value: usize) -> usize {
    value.div_ceil(2)
}

fn digest(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

struct Planes {
    width: usize,
    height: usize,
    y: Vec<u8>,
    u: Vec<u8>,
    v: Vec<u8>,
}

impl Planes {
    fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            y: vec![0; width * height],
            u: vec![0; half(width) * half(height)],
            v: vec![0; half(width) * half(height)],
        }
    }

    fn random(width: usize, height: usize) -> Self {
        let mut rng = Rng::new();
        let mut planes = Self::new(width, height);
        rng.fill(&mut planes.y);
        rng.fill(&mut planes.u);
        rng.fill(&mut planes.v);
        planes
    }

    fn strides(&self) -> Strides {
        Strides {
            y: self.width,
            u: half(self.width),
            v: half(self.width),
        }
    }

    fn source(&self) -> I420<'_> {
        I420::new(&self.y, &self.u, &self.v, self.strides(), self.width, self.height).unwrap()
    }

    fn destination(&mut self) -> I420Mut<'_> {
        let strides = self.strides();
        I420Mut::new(
            &mut self.y,
            &mut self.u,
            &mut self.v,
            strides,
            self.width,
            self.height,
        )
        .unwrap()
    }

    fn bytes(&self) -> Vec<u8> {
        [&self.y[..], &self.u[..], &self.v[..]].concat()
    }
}

fn scaled(src: (usize, usize), dst: (usize, usize), blend: BlendMode) -> String {
    let source = Planes::random(src.0, src.1);
    let mut target = Planes::new(dst.0, dst.1);
    let settings = ScaleSettings {
        blend,
        ..ScaleSettings::default()
    };
    Scaler::new(settings)
        .i420_scale(&source.source(), &mut target.destination(), FilterMode::Bilinear)
        .unwrap();
    digest(&target.bytes())
}

type Dimensions = (usize, usize);
type ScaleCase = (&'static str, Dimensions, Dimensions, &'static str, &'static str);

const SCALES: [ScaleCase; 11] = [
    (
        "down by 2",
        (640, 480),
        (320, 240),
        "3f8e1de2e692dd5b42ca8de948c52284d2fc2d3e100ea947bfdd1dc52bb64e49",
        "3f8e1de2e692dd5b42ca8de948c52284d2fc2d3e100ea947bfdd1dc52bb64e49",
    ),
    (
        "down to 3/4",
        (640, 480),
        (480, 360),
        "219f3b51a6228cea71edaa93a8e4a3a459d90017d672602d71ead42376f06766",
        "219f3b51a6228cea71edaa93a8e4a3a459d90017d672602d71ead42376f06766",
    ),
    (
        "down to 3/8",
        (640, 480),
        (240, 180),
        "0b030b15d743e439596421687ad584437c8220fe541ab92644376a9d20b93624",
        "0b030b15d743e439596421687ad584437c8220fe541ab92644376a9d20b93624",
    ),
    (
        "down by 2 with another aspect",
        (1280, 720),
        (640, 480),
        "8d57c9b6fa4c8076a3576bea898241d82bb3a3456f98c9f537267e36272c22e1",
        "8d57c9b6fa4c8076a3576bea898241d82bb3a3456f98c9f537267e36272c22e1",
    ),
    (
        "down with odd planes",
        (333, 222),
        (200, 100),
        "6f9c7a82b8910be0ca5079229ca6554af363bc984beb91db87292ec2d1b6151f",
        "d74f6f0ec44482bb111ba49df4230b0d689a6acb54d6a3a679ea33b2d0354075",
    ),
    (
        "up with odd planes",
        (100, 75),
        (150, 113),
        "33e65f0411aca2c17b51ab07987d2e621e9132514e1e7ca79c82adfc626163ce",
        "76cb63c1632c0bcada11187b9c296cc99631045ddffa7d66956ccd8609c23536",
    ),
    (
        "down by 3 without filter",
        (480, 360),
        (160, 120),
        "73aab19f88cb334002e61f6657f3980cc73141ea7d0a79c1118a628f54e1e871",
        "73aab19f88cb334002e61f6657f3980cc73141ea7d0a79c1118a628f54e1e871",
    ),
    (
        "up by 2",
        (320, 240),
        (640, 480),
        "098ff68c600d25f645b3dfde4c8fedd77abc71dee7adf05a5068c9e41c043c66",
        "098ff68c600d25f645b3dfde4c8fedd77abc71dee7adf05a5068c9e41c043c66",
    ),
    (
        "up by 2 horizontally",
        (320, 240),
        (640, 240),
        "46d3a951f306cad7c9471e3c6206fe4888f92d795cf1ef2e743123a2d956593c",
        "46d3a951f306cad7c9471e3c6206fe4888f92d795cf1ef2e743123a2d956593c",
    ),
    (
        "up by a fraction",
        (320, 240),
        (512, 384),
        "696401c4dbb9e6e0c5de17c7369a6bd627ed5478fd08314696498b9f1473325a",
        "cf2c0a3b7e1f533ff2c81e611a928d5e5f7ef2da8ff4bbe6e9be5ada501bb8ff",
    ),
    (
        "down vertically",
        (640, 480),
        (640, 240),
        "aed585ac9cfd67e07d33cdde2e3d6f39a36eb3e91b93ba5fc298d8f69e4a38ab",
        "aed585ac9cfd67e07d33cdde2e3d6f39a36eb3e91b93ba5fc298d8f69e4a38ab",
    ),
];

#[test]
fn scaling_with_the_intel_blender_matches_libyuv() {
    for (name, src, dst, intel, _) in SCALES {
        assert_eq!(scaled(src, dst, BlendMode::Intel7), intel, "{name}");
    }
}

#[test]
fn scaling_with_the_arm_blender_matches_libyuv() {
    for (name, src, dst, _, arm) in SCALES {
        assert_eq!(scaled(src, dst, BlendMode::Arm16), arm, "{name}");
    }
}

fn yuv_conversions(width: usize, height: usize, invert: bool) -> (String, String) {
    let source = Planes::random(width, height);
    let converter = Converter::default();

    let mut rgb565 = vec![0u8; width * 2 * height];
    let mut dst = PlaneMut::new(&mut rgb565, width * 2, width * 2, height).unwrap();
    converter
        .i420_to_rgb565(&source.source(), &mut dst, invert)
        .unwrap();

    let yuy2_stride = half(width) * 4;
    let mut yuy2 = vec![0u8; yuy2_stride * height];
    let mut dst = PlaneMut::new(&mut yuy2, yuy2_stride, yuy2_stride, height).unwrap();
    converter
        .i420_to_yuy2(&source.source(), &mut dst, invert)
        .unwrap();

    (digest(&rgb565), digest(&yuy2))
}

#[test]
fn yuv_conversions_match_libyuv() {
    let cases = [
        (
            (64, 48, false),
            "f06ffaf672ca2d32968fee58c2ec0f265f7f72c14a28dc7b35f990ae0d820af3",
            "3cd60b5ab060892a7f4ec2defea3aa829097a9fd4e73da7a7eb5c8febd19f9dc",
        ),
        (
            (64, 48, true),
            "a9da044c72b131e2a7d75927ee2562ae325c27b4578c9ee2b5d4b258bc9a0782",
            "bacf3994b79ad3e9d1c091e17cfc464c27e3866aa67b7d9812351dbf77cdae2a",
        ),
        (
            (33, 27, false),
            "d1a2e48270d664cbb913f642751d0d2451e702db8241d662ea729edff27433c8",
            "4be8e388c0f6fcef7eef80256d255043945765a065f68f3f71a1310fb25a2c20",
        ),
        (
            (33, 27, true),
            "718c32c6aee12ca0b3f13ebc4a7b5456b47c88605814643b1f1e4d51a6776445",
            "67082d6ddace68dbed81d05018d65342ac204ccec22968e0efd6406fe378b118",
        ),
    ];
    for ((width, height, invert), rgb565, yuy2) in cases {
        let (actual_rgb565, actual_yuy2) = yuv_conversions(width, height, invert);
        assert_eq!(actual_rgb565, rgb565, "{width}x{height} inverted={invert} rgb565");
        assert_eq!(actual_yuy2, yuy2, "{width}x{height} inverted={invert} yuy2");
    }
}

fn argb_source(height: usize, stride: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; stride * height];
    Rng::new().fill(&mut bytes);
    bytes
}

fn argb_conversions(
    width: usize,
    height: usize,
    stride: usize,
    invert: bool,
) -> (String, String) {
    let bytes = argb_source(height, stride);
    let source = Argb::new(&bytes, stride, width, height).unwrap();
    let converter = Converter::default();

    let mut rgb565 = vec![0u8; width * 2 * height];
    let mut dst = PlaneMut::new(&mut rgb565, width * 2, width * 2, height).unwrap();
    converter.argb_to_rgb565(&source, &mut dst, invert).unwrap();

    let yuy2_stride = half(width) * 4;
    let mut yuy2 = vec![0u8; yuy2_stride * height];
    let mut dst = PlaneMut::new(&mut yuy2, yuy2_stride, yuy2_stride, height).unwrap();
    converter.argb_to_yuy2(&source, &mut dst, invert).unwrap();

    (digest(&rgb565), digest(&yuy2))
}

#[test]
fn argb_conversions_match_libyuv() {
    let cases = [
        (
            (64, 48, 64 * 4, false),
            "95bb89be5005316d030e29b8443622c50806f386cf00db255db0fbf8a9aee1c2",
            "acb8181b444cb559d1fe540906301a821690e503f80296bc3509619dff2d6245",
        ),
        (
            (64, 48, 64 * 4, true),
            "25f8402d0468088268e0227fb3bfdbc471fa5f0542c6159e101f8b1f6658cf6a",
            "3ac83e62361b81f9b7255ce2d361d394ac6192723adc548f99eb454d50ea0d70",
        ),
        (
            (33, 27, 33 * 4 + 8, false),
            "aa8cc3a3e0bb98e2d0b7f338c2075b12c080e5edfd8982c3646c0c908202bc58",
            "67f0549735fe2453a0dbe11d040520d960572dfc3b1cdf2751fbb6605eb54a57",
        ),
        (
            (33, 27, 33 * 4 + 8, true),
            "00abcad0e28785987be242a109b598395ccc10b732004eb9e1dba91f3f61b095",
            "b65c102cd872f775a9b4636aaee1f2ed136fdc8e17c6d16b8063fe1eac9db6d1",
        ),
    ];
    for ((width, height, stride, invert), rgb565, yuy2) in cases {
        let (actual_rgb565, actual_yuy2) = argb_conversions(width, height, stride, invert);
        assert_eq!(actual_rgb565, rgb565, "{width}x{height} inverted={invert} rgb565");
        assert_eq!(actual_yuy2, yuy2, "{width}x{height} inverted={invert} yuy2");
    }
}

#[test]
fn argb_swaps_and_mirrors_match_libyuv() {
    let (width, height, stride) = (33, 27, 33 * 4 + 8);
    let bytes = argb_source(height, stride);
    let source = Argb::new(&bytes, stride, width, height).unwrap();

    let mut swapped = vec![0u8; width * 4 * height];
    let mut dst = PlaneMut::new(&mut swapped, width * 4, width * 4, height).unwrap();
    abgr_to_argb(&source, &mut dst).unwrap();
    assert_eq!(
        digest(&swapped),
        "3d2d514632ca0a51d7690a3778bcc7f7543b08b24398f4db491b2d27656e8348"
    );

    let mut mirrored = vec![0u8; width * 4 * height];
    let mut dst = PlaneMut::new(&mut mirrored, width * 4, width * 4, height).unwrap();
    argb_mirror(&source, &mut dst).unwrap();
    assert_eq!(
        digest(&mirrored),
        "ad3929c7da51c7356514b8b8b35ac771be30b5b8826cff8ddba259e2db5a4086"
    );
}

#[test]
fn mirroring_and_rotating_match_libyuv() {
    let cases = [
        (
            (33, 27),
            "bf88900cb84b80f481029228703542cfab30656840f803e17f4ce9b6806604fd",
            [
                "bfd4a28e6562d4cfa27a379072ba25ae7621b2952eba25b0fc609204f2920f97",
                "5845409a1952d7c99c30d51608253e40b9075109288405acaf21d107777e1306",
                "419432318feed1328c19231d8a2cb19c85acfd1a36780bf7f60d6b02dae6aa53",
                "2bfac61cdbd897def0f9003ee996dc253bb0901887a7fc9e03360ba5e99dc600",
            ],
        ),
        (
            (64, 48),
            "308380a76b146e37b17106aeccf7aa38d2bccb005d6810c5614e14a0aecdcfc2",
            [
                "74b880e35a2eea6ca416acc41a1a2ff89a8c7de2cbe33cc3e583f22cdf4f38d3",
                "85d94466c7007f7c277e24b251381b9f0b9bfee27b4f23b50a5a94ea258b3ebb",
                "2e7d42fdccb69c413f02520394fa482e510e44f81bea31337617a88f1496f11b",
                "18070d5295a12f69d04a057694f4880560bf626624bc1ceb0eb1adc39ded835b",
            ],
        ),
    ];
    let modes = [
        RotationMode::Rotate0,
        RotationMode::Rotate90,
        RotationMode::Rotate180,
        RotationMode::Rotate270,
    ];
    for ((width, height), mirror, rotations) in cases {
        let source = Planes::random(width, height);

        let mut mirrored = Planes::new(width, height);
        i420_mirror(&source.source(), &mut mirrored.destination()).unwrap();
        assert_eq!(digest(&mirrored.bytes()), mirror, "{width}x{height} mirror");

        for (mode, expected) in modes.into_iter().zip(rotations) {
            let swaps = matches!(mode, RotationMode::Rotate90 | RotationMode::Rotate270);
            let (rotated_width, rotated_height) = if swaps { (height, width) } else { (width, height) };
            let mut rotated = Planes::new(rotated_width, rotated_height);
            i420_rotate(&source.source(), &mut rotated.destination(), mode).unwrap();
            assert_eq!(digest(&rotated.bytes()), expected, "{width}x{height} {mode:?}");
        }
    }
}

#[test]
fn android_frames_are_converted_like_libyuv() {
    let cases = [
        (
            (64, 48, 1),
            "74b880e35a2eea6ca416acc41a1a2ff89a8c7de2cbe33cc3e583f22cdf4f38d3",
        ),
        (
            (64, 48, 2),
            "d4a423513a5624336fbacf086646f66a4e06c7f34236a47e3e0437f8b60bee1f",
        ),
        (
            (33, 27, 2),
            "1f4f45985910c993253478e8e477b6fae4e3f7f1d80ffcb2a9fe4249e4b9f287",
        ),
    ];
    for ((width, height, pixel_stride), expected) in cases {
        let (half_width, half_height) = (half(width), half(height));
        let chroma_stride = if pixel_stride == 1 {
            half_width
        } else {
            (half_width - 1) * pixel_stride + 1 + 3
        };
        let mut rng = Rng::new();
        let mut y = vec![0u8; width * height];
        let mut u = vec![0u8; chroma_stride * half_height];
        let mut v = vec![0u8; chroma_stride * half_height];
        rng.fill(&mut y);
        rng.fill(&mut u);
        rng.fill(&mut v);

        let source = Android420::new(
            &y,
            &u,
            &v,
            Android420Strides {
                y: width,
                u: chroma_stride,
                v: chroma_stride,
                pixel_uv: pixel_stride,
            },
            width,
            height,
        )
        .unwrap();
        let mut target = Planes::new(width, height);
        android420_to_i420(&source, &mut target.destination()).unwrap();
        assert_eq!(digest(&target.bytes()), expected, "{width}x{height} pixel stride {pixel_stride}");
    }
}
