use serde::{Deserialize, Serialize};

#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl std::ops::Add for Vec3 {
    type Output = Self;
    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y, self.z + other.z)
    }
}

impl std::ops::Sub for Vec3 {
    type Output = Self;
    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y, self.z - other.z)
    }
}

impl Vec3 {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0);
    pub const UP: Self = Self::new(0.0, 1.0, 0.0);
    pub const RIGHT: Self = Self::new(1.0, 0.0, 0.0);
    pub const FORWARD: Self = Self::new(0.0, 0.0, 1.0);

    pub fn from_array(a: [f32; 3]) -> Self {
        Self::new(a[0], a[1], a[2])
    }

    pub fn to_array(self) -> [f32; 3] {
        [self.x, self.y, self.z]
    }

    pub fn dot(self, other: Self) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn cross(self, other: Self) -> Self {
        Self::new(
            self.y * other.z - self.z * other.y,
            self.z * other.x - self.x * other.z,
            self.x * other.y - self.y * other.x,
        )
    }

    pub fn length_squared(self) -> f32 {
        self.dot(self)
    }

    pub fn length(self) -> f32 {
        self.length_squared().sqrt()
    }

    pub fn normalize(self) -> Self {
        let len = self.length();
        if len > 0.00001 {
            Self::new(self.x / len, self.y / len, self.z / len)
        } else {
            Self::ZERO
        }
    }

    pub fn scale(self, s: f32) -> Self {
        Self::new(self.x * s, self.y * s, self.z * s)
    }
}

#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Vec4 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

impl Vec4 {
    pub const fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { x, y, z, w }
    }

    pub fn from_vec3(v: Vec3, w: f32) -> Self {
        Self::new(v.x, v.y, v.z, w)
    }

    pub fn to_vec3_perspective_divide(self) -> Vec3 {
        if self.w.abs() > 0.00001 {
            Vec3::new(self.x / self.w, self.y / self.w, self.z / self.w)
        } else {
            Vec3::new(self.x, self.y, self.z)
        }
    }
}

/// 4x4 column-major matrix
#[must_use]
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Mat4 {
    pub m: [f32; 16],
}

impl Mat4 {
    pub const IDENTITY: Self = Self {
        m: [
            1.0, 0.0, 0.0, 0.0, // col 0
            0.0, 1.0, 0.0, 0.0, // col 1
            0.0, 0.0, 1.0, 0.0, // col 2
            0.0, 0.0, 0.0, 1.0, // col 3
        ],
    };

    pub fn multiply(self, other: Self) -> Self {
        let mut out = [0.0; 16];
        for col in 0..4 {
            for row in 0..4 {
                let mut sum = 0.0;
                for k in 0..4 {
                    sum += self.m[k * 4 + row] * other.m[col * 4 + k];
                }
                out[col * 4 + row] = sum;
            }
        }
        Self { m: out }
    }

    pub fn translation(t: Vec3) -> Self {
        Self {
            m: [
                1.0, 0.0, 0.0, 0.0, //
                0.0, 1.0, 0.0, 0.0, //
                0.0, 0.0, 1.0, 0.0, //
                t.x, t.y, t.z, 1.0, //
            ],
        }
    }

    pub fn scale(s: Vec3) -> Self {
        Self {
            m: [
                s.x, 0.0, 0.0, 0.0, //
                0.0, s.y, 0.0, 0.0, //
                0.0, 0.0, s.z, 0.0, //
                0.0, 0.0, 0.0, 1.0, //
            ],
        }
    }

    pub fn rotation_y(angle_rad: f32) -> Self {
        let (sin, cos) = angle_rad.sin_cos();
        Self {
            m: [
                cos, 0.0, -sin, 0.0, //
                0.0, 1.0, 0.0, 0.0, //
                sin, 0.0, cos, 0.0, //
                0.0, 0.0, 0.0, 1.0, //
            ],
        }
    }

    pub fn rotation_x(angle_rad: f32) -> Self {
        let (sin, cos) = angle_rad.sin_cos();
        Self {
            m: [
                1.0, 0.0, 0.0, 0.0, //
                0.0, cos, sin, 0.0, //
                0.0, -sin, cos, 0.0, //
                0.0, 0.0, 0.0, 1.0, //
            ],
        }
    }

    pub fn rotation_z(angle_rad: f32) -> Self {
        let (sin, cos) = angle_rad.sin_cos();
        Self {
            m: [
                cos, sin, 0.0, 0.0, //
                -sin, cos, 0.0, 0.0, //
                0.0, 0.0, 1.0, 0.0, //
                0.0, 0.0, 0.0, 1.0, //
            ],
        }
    }

    pub fn perspective(fov_y_rad: f32, aspect: f32, near: f32, far: f32) -> Self {
        let tan_half = (fov_y_rad * 0.5).tan();
        let f = 1.0 / tan_half;

        let m00 = f / aspect;
        let m11 = f;
        let m22 = (far + near) / (near - far);
        let m23 = -1.0;
        let m32 = (2.0 * far * near) / (near - far);

        Self {
            m: [
                m00, 0.0, 0.0, 0.0, //
                0.0, m11, 0.0, 0.0, //
                0.0, 0.0, m22, m23, //
                0.0, 0.0, m32, 0.0, //
            ],
        }
    }

    pub fn look_at(eye: Vec3, target: Vec3, up: Vec3) -> Self {
        let f = (target - eye).normalize();
        let r = f.cross(up).normalize();
        let u = r.cross(f);

        Self {
            m: [
                r.x,
                u.x,
                -f.x,
                0.0,
                r.y,
                u.y,
                -f.y,
                0.0,
                r.z,
                u.z,
                -f.z,
                0.0,
                -r.dot(eye),
                -u.dot(eye),
                f.dot(eye),
                1.0,
            ],
        }
    }

    pub fn transform_vec4(self, v: Vec4) -> Vec4 {
        Vec4::new(
            self.m[0] * v.x + self.m[4] * v.y + self.m[8] * v.z + self.m[12] * v.w,
            self.m[1] * v.x + self.m[5] * v.y + self.m[9] * v.z + self.m[13] * v.w,
            self.m[2] * v.x + self.m[6] * v.y + self.m[10] * v.z + self.m[14] * v.w,
            self.m[3] * v.x + self.m[7] * v.y + self.m[11] * v.z + self.m[15] * v.w,
        )
    }

    pub fn transform_point(self, p: Vec3) -> Vec3 {
        self.transform_vec4(Vec4::from_vec3(p, 1.0))
            .to_vec3_perspective_divide()
    }

    pub fn transform_normal(self, n: Vec3) -> Vec3 {
        // Linear 3x3 rotation transformation
        Vec3::new(
            self.m[0] * n.x + self.m[4] * n.y + self.m[8] * n.z,
            self.m[1] * n.x + self.m[5] * n.y + self.m[9] * n.z,
            self.m[2] * n.x + self.m[6] * n.y + self.m[10] * n.z,
        )
        .normalize()
    }
}
