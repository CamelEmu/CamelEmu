use crate::context::ShaderProgram;

pub struct CameraSettings {
    pub opcode_shift: u32,
    pub dp4_opcode: u32,
    pub dph_opcode: u32,
    pub dest_shift: u32,
    pub dest_mask: u32,
    pub position_output: u32,
    pub src1_shift: u32,
    pub src1_mask: u32,
    pub first_float_uniform: u32,
    pub matrix_rows: usize,
    pub fallback_projection: Projection,
}

impl Default for CameraSettings {
    fn default() -> Self {
        Self {
            opcode_shift: 26,
            dp4_opcode: 0x02,
            dph_opcode: 0x03,
            dest_shift: 21,
            dest_mask: 0x1f,
            position_output: 0,
            src1_shift: 12,
            src1_mask: 0x7f,
            first_float_uniform: 0x20,
            matrix_rows: 4,
            fallback_projection: Projection {
                yfov: 0.8,
                aspect: 400.0 / 240.0,
                near: 0.1,
                far: 1000.0,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraData {
    pub position: [f32; 3],
    pub rotation: [f32; 4],
    pub projection: Projection,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projection {
    pub yfov: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for CameraData {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            projection: CameraSettings::default().fallback_projection,
        }
    }
}

impl CameraData {
    pub fn estimate(program: &ShaderProgram, settings: &CameraSettings) -> Option<Self> {
        let mut rows = Vec::with_capacity(settings.matrix_rows);
        for word in &program.code {
            let opcode = word >> settings.opcode_shift;
            let dest = (word >> settings.dest_shift) & settings.dest_mask;
            let src1 = (word >> settings.src1_shift) & settings.src1_mask;
            if (opcode != settings.dp4_opcode && opcode != settings.dph_opcode)
                || dest != settings.position_output
                || src1 < settings.first_float_uniform
            {
                continue;
            }
            let index = (src1 - settings.first_float_uniform) as usize;
            if !rows.contains(&index) {
                rows.push(index);
            }
        }
        rows.sort_unstable();
        let first = *rows.first()?;
        let last = settings.matrix_rows.checked_sub(1)?;
        if rows.get(last) != Some(&(first + last)) {
            return None;
        }
        let matrix: [[f32; 4]; 4] = program
            .uniforms
            .get(first..first + settings.matrix_rows)?
            .try_into()
            .ok()?;
        Self::from_clip_matrix(matrix, settings)
    }

    pub fn from_clip_matrix(rows: [[f32; 4]; 4], settings: &CameraSettings) -> Option<Self> {
        let [r0, r1, r2, r3] = rows;
        let back = normalize(neg(xyz(r3)))?;
        let right = normalize(xyz(r0))?;
        let up = normalize(cross(back, right))?;
        let right = cross(up, back);
        let position = solve([xyz(r0), xyz(r1), xyz(r3)], [-r0[3], -r1[3], -r3[3]])?;

        let p00 = length(xyz(r0));
        let p11 = length(xyz(r1));
        let a = dot(xyz(r2), back);
        let b = r2[3] + a * r3[3];
        let near = b / (a - 1.0);
        let far = b / a;
        let fallback = settings.fallback_projection;
        let (near, far) = if near.is_finite() && far.is_finite() && 0.0 < near && near < far {
            (near, far)
        } else {
            (fallback.near, fallback.far)
        };

        Some(Self {
            position,
            rotation: quaternion(right, up, back),
            projection: Projection {
                yfov: 2.0 * (1.0 / p11).atan(),
                aspect: p11 / p00,
                near,
                far,
            },
        })
    }
}

fn xyz(row: [f32; 4]) -> [f32; 3] {
    [row[0], row[1], row[2]]
}

fn neg(v: [f32; 3]) -> [f32; 3] {
    [-v[0], -v[1], -v[2]]
}

fn dot(a: [f32; 3], b: [f32; 3]) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn length(v: [f32; 3]) -> f32 {
    dot(v, v).sqrt()
}

fn normalize(v: [f32; 3]) -> Option<[f32; 3]> {
    let len = length(v);
    (len.is_finite() && len > f32::EPSILON).then(|| [v[0] / len, v[1] / len, v[2] / len])
}

fn solve(rows: [[f32; 3]; 3], rhs: [f32; 3]) -> Option<[f32; 3]> {
    let det = |m: [[f32; 3]; 3]| dot(m[0], cross(m[1], m[2]));
    let d = det(rows);
    if !d.is_finite() || d.abs() < f32::EPSILON {
        return None;
    }
    let mut out = [0.0; 3];
    for (column, value) in out.iter_mut().enumerate() {
        let mut m = rows;
        for (row, r) in m.iter_mut().zip(rhs) {
            row[column] = r;
        }
        *value = det(m) / d;
    }
    Some(out)
}

fn quaternion(x: [f32; 3], y: [f32; 3], z: [f32; 3]) -> [f32; 4] {
    let (m00, m10, m20) = (x[0], x[1], x[2]);
    let (m01, m11, m21) = (y[0], y[1], y[2]);
    let (m02, m12, m22) = (z[0], z[1], z[2]);
    let trace = m00 + m11 + m22;
    if trace > 0.0 {
        let s = (trace + 1.0).sqrt() * 2.0;
        [(m21 - m12) / s, (m02 - m20) / s, (m10 - m01) / s, 0.25 * s]
    } else if m00 > m11 && m00 > m22 {
        let s = (1.0 + m00 - m11 - m22).sqrt() * 2.0;
        [0.25 * s, (m01 + m10) / s, (m02 + m20) / s, (m21 - m12) / s]
    } else if m11 > m22 {
        let s = (1.0 + m11 - m00 - m22).sqrt() * 2.0;
        [(m01 + m10) / s, 0.25 * s, (m12 + m21) / s, (m02 - m20) / s]
    } else {
        let s = (1.0 + m22 - m00 - m11).sqrt() * 2.0;
        [(m02 + m20) / s, (m12 + m21) / s, 0.25 * s, (m10 - m01) / s]
    }
}
