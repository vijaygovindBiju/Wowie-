use crate::math::{Mat4, Vec3, Vec4};
use std::f32::consts::PI;
use wowie_avatar::model::{CompanionMood, Model3D};

#[derive(Debug, Clone)]
pub struct Camera3D {
    pub target: Vec3,
    pub distance: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub fov_y_rad: f32,
}

impl Default for Camera3D {
    fn default() -> Self {
        Self {
            target: Vec3::new(0.0, 0.0, 0.0),
            distance: 3.5,
            yaw: 0.15,
            pitch: 0.12,
            fov_y_rad: 45.0 * PI / 180.0,
        }
    }
}

impl Camera3D {
    pub fn view_matrix(&self) -> Mat4 {
        let (sy, cy) = self.yaw.sin_cos();
        let (sp, cp) = self.pitch.sin_cos();

        let eye_offset = Vec3::new(
            self.distance * sy * cp,
            self.distance * sp,
            self.distance * cy * cp,
        );
        let eye = self.target + eye_offset;
        Mat4::look_at(eye, self.target, Vec3::UP)
    }

    pub fn projection_matrix(&self, aspect: f32) -> Mat4 {
        Mat4::perspective(self.fov_y_rad, aspect, 0.1, 100.0)
    }
}

#[derive(Debug, Clone)]
pub struct RenderStats {
    pub frame_count: u64,
    pub rendered_triangles: usize,
    pub rendered_vertices: usize,
    pub fps: f32,
}

#[derive(Debug)]
pub struct Renderer3D {
    pub width: usize,
    pub height: usize,
    pub camera: Camera3D,
    pub depth_buffer: Vec<f32>,
    pub color_buffer: Vec<u32>,
    pub frame_counter: u64,
}

impl Renderer3D {
    pub fn new(width: usize, height: usize) -> Self {
        let pixel_count = width * height;
        Self {
            width,
            height,
            camera: Camera3D::default(),
            depth_buffer: vec![f32::INFINITY; pixel_count],
            color_buffer: vec![0u32; pixel_count],
            frame_counter: 0,
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        if width == self.width && height == self.height {
            return;
        }
        self.width = width;
        self.height = height;
        let pixel_count = width * height;
        self.depth_buffer = vec![f32::INFINITY; pixel_count];
        self.color_buffer = vec![0u32; pixel_count];
    }

    pub fn clear(&mut self) {
        // Clear to transparent ARGB (alpha = 0)
        self.color_buffer.fill(0x0000_0000);
        self.depth_buffer.fill(f32::INFINITY);
    }

    /// Computes model transform matrix expressing the companion state animation
    pub fn compute_animation_transform(&self, mood: CompanionMood, elapsed_secs: f32) -> Mat4 {
        match mood {
            CompanionMood::Neutral => {
                // Idle: gentle breathing bobbing and subtle sway
                let bob_y = (elapsed_secs * 1.5).sin() * 0.06;
                let sway_yaw = (elapsed_secs * 0.8).sin() * 0.08;
                let breath_scale = 1.0 + (elapsed_secs * 1.5).sin() * 0.02;

                let t = Mat4::translation(Vec3::new(0.0, bob_y, 0.0));
                let r = Mat4::rotation_y(sway_yaw);
                let s = Mat4::scale(Vec3::new(breath_scale, breath_scale, breath_scale));
                t.multiply(r).multiply(s)
            }
            CompanionMood::Focused => {
                // Deep Focus: attentive posture, subtle micro-bob, slight forward tilt
                let micro_bob = (elapsed_secs * 3.0).sin() * 0.015;
                let t = Mat4::translation(Vec3::new(0.0, micro_bob, 0.0));
                let r_pitch = Mat4::rotation_x(0.05); // slight forward attentiveness
                t.multiply(r_pitch)
            }
            CompanionMood::Concerned => {
                // Distraction Alert: agitated pulse and attention-grabbing warning roll
                let alert_bob = (elapsed_secs * 7.0).sin() * 0.07;
                let alert_roll = (elapsed_secs * 9.0).sin() * 0.10;
                let t = Mat4::translation(Vec3::new(0.0, alert_bob, 0.0));
                let r = Mat4::rotation_z(alert_roll);
                t.multiply(r)
            }
            CompanionMood::Relaxed => {
                // Snoozed / Break: sleepy drooping tilt and slow deep breathing
                let sleep_breath = (elapsed_secs * 0.8).sin() * 0.03;
                let t = Mat4::translation(Vec3::new(0.0, sleep_breath - 0.05, 0.0));
                let r = Mat4::rotation_z(-0.18); // drooping tilt
                t.multiply(r)
            }
            CompanionMood::Celebrating => {
                // Celebration: energetic bouncing hops and joyful spin
                let hop_y = (elapsed_secs * 4.0).sin().abs() * 0.22;
                let spin_yaw = elapsed_secs * 2.8;
                let t = Mat4::translation(Vec3::new(0.0, hop_y, 0.0));
                let r = Mat4::rotation_y(spin_yaw);
                t.multiply(r)
            }
            CompanionMood::Thinking => {
                // Thinking / Analyzing: curious tilt and searching oscillation
                let tilt = (elapsed_secs * 2.0).sin() * 0.08;
                let sway = (elapsed_secs * 1.2).cos() * 0.05;
                let t = Mat4::translation(Vec3::new(0.0, sway, 0.0));
                let r = Mat4::rotation_z(tilt);
                t.multiply(r)
            }
            CompanionMood::Snoozed => {
                // Snoozed: slow peaceful breathing, sleepy tilt, gentle bobbing
                let sleep_breath = (elapsed_secs * 0.7).sin() * 0.03;
                let t = Mat4::translation(Vec3::new(0.0, sleep_breath - 0.05, 0.0));
                let r = Mat4::rotation_z(-0.20);
                t.multiply(r)
            }
        }
    }

    /// Render a real 3D model with true perspective projection, depth-testing, and dynamic mood lighting.
    pub fn render_model(
        &mut self,
        model: &Model3D,
        mood: CompanionMood,
        elapsed_secs: f32,
    ) -> RenderStats {
        self.clear();
        self.frame_counter += 1;

        if model.is_empty() || self.width == 0 || self.height == 0 {
            return RenderStats {
                frame_count: self.frame_counter,
                rendered_triangles: 0,
                rendered_vertices: 0,
                fps: 30.0,
            };
        }

        let aspect = (self.width as f32) / (self.height as f32);
        let proj = self.camera.projection_matrix(aspect);
        let view = self.camera.view_matrix();
        let anim_model = self.compute_animation_transform(mood, elapsed_secs);

        let mv = view.multiply(anim_model);
        let mvp = proj.multiply(mv);

        // Dynamic lighting colors based on mood
        let (light_dir, light_color, ambient_color) = match mood {
            CompanionMood::Neutral => (
                Vec3::new(0.6, 0.8, 0.7).normalize(),
                [0.9, 0.92, 0.98],
                [0.25, 0.28, 0.35],
            ),
            CompanionMood::Focused => (
                Vec3::new(0.4, 0.9, 0.6).normalize(),
                [0.7, 0.95, 1.0], // cyan-tinted focus aura
                [0.2, 0.35, 0.45],
            ),
            CompanionMood::Concerned => {
                // Pulsing red/amber alert light
                let pulse = (elapsed_secs * 8.0).sin() * 0.5 + 0.5;
                (
                    Vec3::new(0.0, 1.0, 0.8).normalize(),
                    [1.0, 0.3 + 0.4 * pulse, 0.2],
                    [0.4 + 0.2 * pulse, 0.15, 0.1],
                )
            }
            CompanionMood::Relaxed => (
                Vec3::new(0.3, 0.7, 0.6).normalize(),
                [0.75, 0.7, 0.95], // gentle twilight purple
                [0.2, 0.18, 0.3],
            ),
            CompanionMood::Celebrating => {
                // Golden celebratory sparkle
                let sparkle = (elapsed_secs * 6.0).sin() * 0.2 + 0.8;
                (
                    Vec3::new(0.5, 0.9, 0.5).normalize(),
                    [1.0 * sparkle, 0.85 * sparkle, 0.3],
                    [0.4, 0.3, 0.15],
                )
            }
            CompanionMood::Thinking => (
                Vec3::new(0.5, 0.8, 0.6).normalize(),
                [0.8, 0.85, 1.0],
                [0.25, 0.3, 0.4],
            ),
            CompanionMood::Snoozed => (
                Vec3::new(0.3, 0.6, 0.7).normalize(),
                [0.6, 0.65, 0.95], // dim restful indigo/lavender
                [0.18, 0.16, 0.28],
            ),
        };

        let mut rendered_triangles = 0;
        let mut rendered_vertices = 0;

        let half_w = self.width as f32 * 0.5;
        let half_h = self.height as f32 * 0.5;

        for mesh in &model.meshes {
            rendered_vertices += mesh.vertices.len();
            let v_len = mesh.vertices.len();

            // Transform vertices to clip space and screen space
            let mut screen_points: Vec<(Vec3, f32, Vec3, [f32; 4])> = Vec::with_capacity(v_len);

            for v in &mesh.vertices {
                let pos = Vec3::from_array(v.position);
                let norm = Vec3::from_array(v.normal);

                let world_norm = anim_model.transform_normal(norm);
                let clip = mvp.transform_vec4(Vec4::from_vec3(pos, 1.0));

                let w = clip.w;
                let ndc_z = if w.abs() > 0.0001 { clip.z / w } else { 1.0 };

                let screen_x = (clip.x / w.max(0.0001) + 1.0) * half_w;
                let screen_y = (1.0 - clip.y / w.max(0.0001)) * half_h;

                screen_points.push((Vec3::new(screen_x, screen_y, ndc_z), w, world_norm, v.color));
            }

            // Rasterize triangles
            let idx_count = mesh.indices.len();
            for i in (0..idx_count).step_by(3) {
                if i + 2 >= idx_count {
                    break;
                }

                let i0 = mesh.indices[i] as usize;
                let i1 = mesh.indices[i + 1] as usize;
                let i2 = mesh.indices[i + 2] as usize;

                if i0 >= v_len || i1 >= v_len || i2 >= v_len {
                    continue;
                }

                let (p0, w0, n0, c0) = screen_points[i0];
                let (p1, w1, n1, c1) = screen_points[i1];
                let (p2, w2, n2, c2) = screen_points[i2];

                // Near-plane clipping
                if w0 <= 0.01 && w1 <= 0.01 && w2 <= 0.01 {
                    continue;
                }

                // Back-face culling in screen space (signed area: negative for CCW front faces with Y-down)
                let edge_area = (p1.x - p0.x) * (p2.y - p0.y) - (p1.y - p0.y) * (p2.x - p0.x);
                let is_front_facing = edge_area < -0.0001;
                let is_back_facing = edge_area > 0.0001;

                if !is_front_facing && !is_back_facing {
                    continue; // Degenerate triangle
                }

                if is_back_facing && !mesh.double_sided {
                    continue; // Cull backface for single-sided meshes
                }

                // If back-facing on double-sided mesh, flip normals for lighting
                let (eff_n0, eff_n1, eff_n2) = if is_back_facing {
                    (n0.scale(-1.0), n1.scale(-1.0), n2.scale(-1.0))
                } else {
                    (n0, n1, n2)
                };

                // Shading calculation at vertices (Gouraud shading)
                let shade_vertex = |n: Vec3, col: [f32; 4]| -> [f32; 3] {
                    let diff = n.dot(light_dir).max(0.0);
                    let r = (ambient_color[0] + light_color[0] * diff) * col[0];
                    let g = (ambient_color[1] + light_color[1] * diff) * col[1];
                    let b = (ambient_color[2] + light_color[2] * diff) * col[2];
                    [r.min(1.0), g.min(1.0), b.min(1.0)]
                };

                let rgb0 = shade_vertex(eff_n0, c0);
                let rgb1 = shade_vertex(eff_n1, c1);
                let rgb2 = shade_vertex(eff_n2, c2);

                self.rasterize_triangle(p0, p1, p2, rgb0, rgb1, rgb2, edge_area);
                rendered_triangles += 1;
            }
        }

        RenderStats {
            frame_count: self.frame_counter,
            rendered_triangles,
            rendered_vertices,
            fps: 30.0,
        }
    }

    fn rasterize_triangle(
        &mut self,
        p0: Vec3,
        p1: Vec3,
        p2: Vec3,
        c0: [f32; 3],
        c1: [f32; 3],
        c2: [f32; 3],
        inv_area: f32,
    ) {
        let inv_area = 1.0 / inv_area;

        // Bounding box
        let min_x = (p0.x.min(p1.x).min(p2.x).floor() as isize).max(0) as usize;
        let max_x = (p0.x.max(p1.x).max(p2.x).ceil() as usize).min(self.width.saturating_sub(1));
        let min_y = (p0.y.min(p1.y).min(p2.y).floor() as isize).max(0) as usize;
        let max_y = (p0.y.max(p1.y).max(p2.y).ceil() as usize).min(self.height.saturating_sub(1));

        if min_x > max_x || min_y > max_y {
            return;
        }

        // Incremental edge steps
        let step_x0 = (p1.y - p2.y) * inv_area;
        let step_x1 = (p2.y - p0.y) * inv_area;
        let min_px = min_x as f32 + 0.5;

        for y in min_y..=max_y {
            let py = y as f32 + 0.5;
            let row_offset = y * self.width;

            let mut w0 = ((p1.x - min_px) * (p2.y - py) - (p1.y - py) * (p2.x - min_px)) * inv_area;
            let mut w1 = ((p2.x - min_px) * (p0.y - py) - (p2.y - py) * (p0.x - min_px)) * inv_area;

            for x in min_x..=max_x {
                let w2 = 1.0 - w0 - w1;

                if w0 >= -0.001 && w1 >= -0.001 && w2 >= -0.001 {
                    let depth = w0 * p0.z + w1 * p1.z + w2 * p2.z;
                    let pixel_idx = row_offset + x;

                    if depth < self.depth_buffer[pixel_idx] {
                        self.depth_buffer[pixel_idx] = depth;

                        let r = (w0 * c0[0] + w1 * c1[0] + w2 * c2[0]).clamp(0.0, 1.0);
                        let g = (w0 * c0[1] + w1 * c1[1] + w2 * c2[1]).clamp(0.0, 1.0);
                        let b = (w0 * c0[2] + w1 * c1[2] + w2 * c2[2]).clamp(0.0, 1.0);

                        let ir = (r * 255.0) as u32;
                        let ig = (g * 255.0) as u32;
                        let ib = (b * 255.0) as u32;

                        // ARGB full opacity for rendered 3D geometry
                        self.color_buffer[pixel_idx] = 0xFF00_0000 | (ir << 16) | (ig << 8) | ib;
                    }
                }

                w0 += step_x0;
                w1 += step_x1;
            }
        }
    }

    /// Draws a modern desktop companion glass pill at the bottom of the overlay
    pub fn draw_hud_pill(
        &mut self,
        _companion_name: &str,
        mood: CompanionMood,
        _status_text: &str,
    ) {
        if self.width < 100 || self.height < 60 {
            return;
        }

        let pill_h = 36;
        let pill_y_start = self.height.saturating_sub(pill_h + 10);
        let pill_x_start = 12;
        let pill_w = self.width.saturating_sub(24);

        let mood_badge_color = match mood {
            CompanionMood::Neutral => 0xFF3B_82F6,     // Blue
            CompanionMood::Focused => 0xFF10_B981,     // Green / Emerald
            CompanionMood::Concerned => 0xFFEF_4444,   // Red
            CompanionMood::Relaxed => 0xFF8B_5CF6,     // Purple
            CompanionMood::Celebrating => 0xFFF5_9E0B, // Amber / Gold
            CompanionMood::Thinking => 0xFF06_B6D4,    // Cyan
            CompanionMood::Snoozed => 0xFF63_66F1,     // Indigo / Lavender
        };

        // Render translucent dark glass pill background
        for y in pill_y_start..(pill_y_start + pill_h) {
            let row = y * self.width;
            for x in pill_x_start..(pill_x_start + pill_w) {
                let idx = row + x;
                if idx < self.color_buffer.len() {
                    // Glass border & background
                    if y == pill_y_start
                        || y == (pill_y_start + pill_h - 1)
                        || x == pill_x_start
                        || x == (pill_x_start + pill_w - 1)
                    {
                        self.color_buffer[idx] = 0xAA6B_7280; // Gray border
                    } else if x < pill_x_start + 6 {
                        self.color_buffer[idx] = mood_badge_color; // Mood indicator bar
                    } else {
                        // Semi-transparent dark background
                        self.color_buffer[idx] = 0xEE1E_293B; // Slate dark
                    }
                }
            }
        }

        // Draw simple bitmap font indicator dot
        let dot_cx = pill_x_start + 14;
        let dot_cy = pill_y_start + pill_h / 2;
        for dy in -3..=3 {
            for dx in -3..=3 {
                if dx * dx + dy * dy <= 9 {
                    let idx = ((dot_cy as isize + dy) as usize) * self.width
                        + ((dot_cx as isize + dx) as usize);
                    if idx < self.color_buffer.len() {
                        self.color_buffer[idx] = mood_badge_color;
                    }
                }
            }
        }
    }
}
