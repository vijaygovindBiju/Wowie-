use crate::error::AvatarError;
use crate::model::{BoundingBox3D, CharacterMetadata, Mesh3D, Model3D, ModelFormat, Vertex3D};
use serde_json::Value;

pub const DEFAULT_MAX_FILE_SIZE_BYTES: u64 = 50 * 1024 * 1024; // 50MB
pub const MAX_JSON_CHUNK_SIZE_BYTES: u64 = 10 * 1024 * 1024; // 10MB
pub const GLB_MAGIC: [u8; 4] = [0x67, 0x6C, 0x54, 0x46]; // "glTF"
pub const JSON_CHUNK_TYPE: [u8; 4] = [0x4A, 0x53, 0x4F, 0x4E]; // "JSON"

const IDENTITY_MAT4: [[f32; 4]; 4] = [
    [1.0, 0.0, 0.0, 0.0],
    [0.0, 1.0, 0.0, 0.0],
    [0.0, 0.0, 1.0, 0.0],
    [0.0, 0.0, 0.0, 1.0],
];

fn multiply_mat4(a: [[f32; 4]; 4], b: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0; 4]; 4];
    for col in 0..4 {
        for row in 0..4 {
            let mut sum = 0.0;
            for k in 0..4 {
                sum += a[k][row] * b[col][k];
            }
            out[col][row] = sum;
        }
    }
    out
}

#[allow(clippy::many_single_char_names)]
fn transform_point(m: [[f32; 4]; 4], p: [f32; 3]) -> [f32; 3] {
    let x = m[0][0] * p[0] + m[1][0] * p[1] + m[2][0] * p[2] + m[3][0];
    let y = m[0][1] * p[0] + m[1][1] * p[1] + m[2][1] * p[2] + m[3][1];
    let z = m[0][2] * p[0] + m[1][2] * p[1] + m[2][2] * p[2] + m[3][2];
    let w = m[0][3] * p[0] + m[1][3] * p[1] + m[2][3] * p[2] + m[3][3];
    if w.abs() > 0.0001 {
        [x / w, y / w, z / w]
    } else {
        [x, y, z]
    }
}

#[allow(clippy::many_single_char_names)]
fn transform_vector(m: [[f32; 4]; 4], v: [f32; 3]) -> [f32; 3] {
    let x = m[0][0] * v[0] + m[1][0] * v[1] + m[2][0] * v[2];
    let y = m[0][1] * v[0] + m[1][1] * v[1] + m[2][1] * v[2];
    let z = m[0][2] * v[0] + m[1][2] * v[1] + m[2][2] * v[2];
    let len = (x * x + y * y + z * z).sqrt();
    if len > 0.00001 {
        [x / len, y / len, z / len]
    } else {
        [0.0, 1.0, 0.0]
    }
}

#[derive(Debug)]
pub struct ModelValidator {
    max_file_size_bytes: u64,
}

impl Default for ModelValidator {
    fn default() -> Self {
        Self {
            max_file_size_bytes: DEFAULT_MAX_FILE_SIZE_BYTES,
        }
    }
}

impl ModelValidator {
    pub fn new(max_file_size_bytes: u64) -> Self {
        Self {
            max_file_size_bytes,
        }
    }

    /// Load, strictly validate, and extract real 3D geometry from a VRM/glTF model.
    pub fn load_3d_model(&self, bytes: &[u8]) -> Result<Model3D, AvatarError> {
        let (metadata, expressions) = self.validate_and_extract(bytes)?;

        let (document, buffers, _images) = gltf::import_slice(bytes).map_err(|e| {
            AvatarError::MalformedGlb(format!("Failed to parse glTF document: {e}"))
        })?;

        let mut extracted_meshes = Vec::new();
        let mut total_triangles = 0;

        let default_scene = document
            .default_scene()
            .or_else(|| document.scenes().next());
        if let Some(scene) = default_scene {
            for node in scene.nodes() {
                Self::traverse_node(
                    &node,
                    IDENTITY_MAT4,
                    &buffers,
                    &mut extracted_meshes,
                    &mut total_triangles,
                );
            }
        }

        // Fallback: If no scene nodes produced meshes, extract directly from document.meshes()
        if extracted_meshes.is_empty() {
            for mesh in document.meshes() {
                Self::extract_mesh_primitives(
                    &mesh,
                    IDENTITY_MAT4,
                    &buffers,
                    &mut extracted_meshes,
                    &mut total_triangles,
                );
            }
        }

        if extracted_meshes.is_empty() {
            return Err(AvatarError::MalformedGlb(
                "Model contains no readable 3D mesh geometry".to_string(),
            ));
        }

        let total_vertices: usize = extracted_meshes.iter().map(|m| m.vertices.len()).sum();
        if total_vertices > 500_000 {
            return Err(AvatarError::SecurityViolation(
                "Model vertex count exceeds safety bound of 500,000 vertices".to_string(),
            ));
        }

        let all_vertices: Vec<Vertex3D> = extracted_meshes
            .iter()
            .flat_map(|m| m.vertices.clone())
            .collect();
        let bounds = BoundingBox3D::from_vertices(&all_vertices);

        let mut model = Model3D {
            metadata,
            available_expressions: expressions,
            meshes: extracted_meshes,
            bounds,
            total_vertex_count: total_vertices,
            total_triangle_count: total_triangles,
        };

        model.normalize_and_center(2.0);
        Ok(model)
    }

    fn traverse_node(
        node: &gltf::Node,
        parent_transform: [[f32; 4]; 4],
        buffers: &[gltf::buffer::Data],
        extracted_meshes: &mut Vec<Mesh3D>,
        total_triangles: &mut usize,
    ) {
        let local_transform = node.transform().matrix();
        let world_transform = multiply_mat4(parent_transform, local_transform);

        if let Some(mesh) = node.mesh() {
            Self::extract_mesh_primitives(
                &mesh,
                world_transform,
                buffers,
                extracted_meshes,
                total_triangles,
            );
        }

        for child in node.children() {
            Self::traverse_node(
                &child,
                world_transform,
                buffers,
                extracted_meshes,
                total_triangles,
            );
        }
    }

    fn extract_mesh_primitives(
        mesh: &gltf::Mesh,
        world_transform: [[f32; 4]; 4],
        buffers: &[gltf::buffer::Data],
        extracted_meshes: &mut Vec<Mesh3D>,
        total_triangles: &mut usize,
    ) {
        let mesh_name = mesh.name().unwrap_or("mesh").to_string();
        let mut mesh_vertices = Vec::new();
        let mut mesh_indices = Vec::new();
        let mut mesh_double_sided = false;

        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));

            let positions: Vec<[f32; 3]> = match reader.read_positions() {
                Some(iter) => iter.collect(),
                None => continue,
            };

            let normals: Vec<[f32; 3]> = reader
                .read_normals()
                .map_or_else(|| vec![[0.0, 1.0, 0.0]; positions.len()], Iterator::collect);

            let material = primitive.material();
            let base_color = material.pbr_metallic_roughness().base_color_factor();
            if material.double_sided() {
                mesh_double_sided = true;
            }

            let colors: Vec<[f32; 4]> = reader.read_colors(0).map_or_else(
                || vec![base_color; positions.len()],
                |iter| {
                    iter.into_rgba_f32()
                        .map(|c| {
                            [
                                c[0] * base_color[0],
                                c[1] * base_color[1],
                                c[2] * base_color[2],
                                c[3] * base_color[3],
                            ]
                        })
                        .collect()
                },
            );

            let base_vertex_offset = mesh_vertices.len() as u32;

            for i in 0..positions.len() {
                let normal = if i < normals.len() {
                    normals[i]
                } else {
                    [0.0, 1.0, 0.0]
                };
                let color = if i < colors.len() {
                    colors[i]
                } else {
                    base_color
                };

                let transformed_pos = transform_point(world_transform, positions[i]);
                let transformed_norm = transform_vector(world_transform, normal);

                mesh_vertices.push(Vertex3D {
                    position: transformed_pos,
                    normal: transformed_norm,
                    color,
                });
            }

            if let Some(indices_iter) = reader.read_indices() {
                let indices: Vec<u32> = indices_iter.into_u32().collect();
                *total_triangles += indices.len() / 3;
                for idx in indices {
                    mesh_indices.push(base_vertex_offset + idx);
                }
            } else {
                for i in 0..(positions.len() as u32) {
                    mesh_indices.push(base_vertex_offset + i);
                }
                *total_triangles += positions.len() / 3;
            }
        }

        if !mesh_vertices.is_empty() {
            extracted_meshes.push(Mesh3D {
                name: mesh_name,
                vertices: mesh_vertices,
                indices: mesh_indices,
                double_sided: mesh_double_sided,
            });
        }
    }

    /// Parse and strictly validate an untrusted model file buffer.
    pub fn validate_and_extract(
        &self,
        bytes: &[u8],
    ) -> Result<(CharacterMetadata, Vec<String>), AvatarError> {
        let actual_size = bytes.len() as u64;
        if actual_size > self.max_file_size_bytes {
            return Err(AvatarError::FileTooLarge {
                actual_bytes: actual_size,
                limit_bytes: self.max_file_size_bytes,
            });
        }

        if bytes.len() < 12 {
            return Err(AvatarError::MalformedGlb(
                "File too small to contain valid GLB header".to_string(),
            ));
        }

        // 1. Validate magic: 0x46546C67 ("glTF")
        let magic: [u8; 4] = bytes[0..4].try_into().unwrap();
        if magic != GLB_MAGIC {
            return Err(AvatarError::InvalidMagic(magic));
        }

        // 2. Validate version
        let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
        if version != 2 {
            return Err(AvatarError::UnsupportedFormat(format!(
                "glTF version {version} unsupported (expected version 2)"
            )));
        }

        // 3. Validate declared total length
        let total_length = u32::from_le_bytes(bytes[8..12].try_into().unwrap()) as usize;
        if total_length > bytes.len() {
            return Err(AvatarError::MalformedGlb(format!(
                "Declared GLB length ({total_length}) exceeds file size ({})",
                bytes.len()
            )));
        }

        // 4. Validate Chunk 0 (JSON)
        if bytes.len() < 20 {
            return Err(AvatarError::MalformedGlb(
                "Truncated GLB: missing chunk 0 header".to_string(),
            ));
        }

        let chunk0_length = u64::from(u32::from_le_bytes(bytes[12..16].try_into().unwrap()));
        let chunk0_type: [u8; 4] = bytes[16..20].try_into().unwrap();

        if chunk0_type != JSON_CHUNK_TYPE {
            return Err(AvatarError::MalformedGlb(format!(
                "First GLB chunk must be JSON, found {chunk0_type:?}"
            )));
        }

        if chunk0_length > MAX_JSON_CHUNK_SIZE_BYTES {
            return Err(AvatarError::SecurityViolation(format!(
                "JSON chunk size ({chunk0_length} bytes) exceeds safety bound of {MAX_JSON_CHUNK_SIZE_BYTES} bytes"
            )));
        }

        let chunk0_end = 20 + chunk0_length as usize;
        if chunk0_end > bytes.len() {
            return Err(AvatarError::MalformedGlb(
                "Truncated GLB: JSON chunk extends past end of file".to_string(),
            ));
        }

        let json_bytes = &bytes[20..chunk0_end];
        let json_val: Value = serde_json::from_slice(json_bytes)
            .map_err(|e| AvatarError::MalformedGlb(format!("Invalid JSON in glTF header: {e}")))?;

        // 5. Security audit of scene graph
        Self::audit_security_constraints(&json_val)?;

        // 6. Extract character metadata and expression list
        let (metadata, expressions) = Self::extract_metadata(&json_val);

        Ok((metadata, expressions))
    }

    fn audit_security_constraints(json: &Value) -> Result<(), AvatarError> {
        // Audit buffers for external URIs
        if let Some(buffers) = json.get("buffers").and_then(Value::as_array) {
            for buf in buffers {
                if let Some(uri) = buf.get("uri").and_then(Value::as_str) {
                    Self::validate_uri(uri)?;
                }
            }
        }

        // Audit images for external URIs
        if let Some(images) = json.get("images").and_then(Value::as_array) {
            for img in images {
                if let Some(uri) = img.get("uri").and_then(Value::as_str) {
                    Self::validate_uri(uri)?;
                }
            }
        }

        // Bound mesh count
        if let Some(meshes) = json.get("meshes").and_then(Value::as_array) {
            if meshes.len() > 256 {
                return Err(AvatarError::SecurityViolation(format!(
                    "Model contains {} meshes, exceeding maximum safety bound of 256",
                    meshes.len()
                )));
            }
        }

        // Reject script execution extensions
        if let Some(exts_required) = json.get("extensionsRequired").and_then(Value::as_array) {
            for ext in exts_required {
                if let Some(name) = ext.as_str() {
                    let lower = name.to_lowercase();
                    if lower.contains("script") || lower.contains("wasm") || lower.contains("exec")
                    {
                        return Err(AvatarError::SecurityViolation(format!(
                            "Unsupported security-sensitive required extension: {name}"
                        )));
                    }
                }
            }
        }

        Ok(())
    }

    fn validate_uri(uri: &str) -> Result<(), AvatarError> {
        let trimmed = uri.trim().to_lowercase();
        if trimmed.starts_with("http://")
            || trimmed.starts_with("https://")
            || trimmed.starts_with("file://")
            || trimmed.starts_with('/')
            || trimmed.contains("..")
        {
            return Err(AvatarError::SecurityViolation(format!(
                "External URI references and path traversals are prohibited: {uri}"
            )));
        }

        // Only embedded data URIs are allowed
        if !trimmed.starts_with("data:") {
            return Err(AvatarError::SecurityViolation(format!(
                "Relative or external file references are prohibited: {uri}"
            )));
        }

        Ok(())
    }

    fn extract_metadata(json: &Value) -> (CharacterMetadata, Vec<String>) {
        let extensions = json.get("extensions");

        // 1. Check for VRM 1.0 (VRMC_vrm)
        if let Some(vrmc) = extensions.and_then(|e| e.get("VRMC_vrm")) {
            let meta = vrmc.get("meta");
            let title = meta
                .and_then(|m| m.get("name"))
                .and_then(Value::as_str)
                .unwrap_or("Custom VRM 1.0 Character")
                .to_string();

            let author = meta
                .and_then(|m| m.get("authors"))
                .and_then(Value::as_array)
                .and_then(|arr| arr.first())
                .and_then(Value::as_str)
                .map(ToString::to_string);

            let version = meta
                .and_then(|m| m.get("version"))
                .and_then(Value::as_str)
                .map(ToString::to_string);

            let license_url = meta
                .and_then(|m| m.get("licenseUrl"))
                .and_then(Value::as_str)
                .map(ToString::to_string);

            let avatar_permission = meta
                .and_then(|m| m.get("avatarPermission"))
                .and_then(Value::as_str)
                .map(ToString::to_string);

            let commercial_usage = meta
                .and_then(|m| m.get("commercialUsage"))
                .and_then(Value::as_str)
                .map(ToString::to_string);

            let mut expressions = vec![
                "neutral".to_string(),
                "happy".to_string(),
                "angry".to_string(),
                "sad".to_string(),
                "relaxed".to_string(),
                "surprised".to_string(),
            ];

            if let Some(expr_obj) = vrmc.get("expressions") {
                if let Some(preset) = expr_obj.get("preset").and_then(Value::as_object) {
                    for key in preset.keys() {
                        if !expressions.contains(key) {
                            expressions.push(key.clone());
                        }
                    }
                }
                if let Some(custom) = expr_obj.get("custom").and_then(Value::as_object) {
                    for key in custom.keys() {
                        if !expressions.contains(key) {
                            expressions.push(key.clone());
                        }
                    }
                }
            }

            let mesh_count = json
                .get("meshes")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);

            return (
                CharacterMetadata {
                    title,
                    author,
                    version,
                    license_name: Some("VRM 1.0 License".to_string()),
                    license_url,
                    avatar_permission,
                    commercial_usage,
                    format: ModelFormat::Vrm1,
                    mesh_count,
                },
                expressions,
            );
        }

        // 2. Check for VRM 0.x (VRM)
        if let Some(vrm0) = extensions.and_then(|e| e.get("VRM")) {
            let meta = vrm0.get("meta");
            let title = meta
                .and_then(|m| m.get("title"))
                .and_then(Value::as_str)
                .unwrap_or("Custom VRM 0.x Character")
                .to_string();

            let author = meta
                .and_then(|m| m.get("author"))
                .and_then(Value::as_str)
                .map(ToString::to_string);

            let version = meta
                .and_then(|m| m.get("version"))
                .and_then(Value::as_str)
                .map(ToString::to_string);

            let license_name = meta
                .and_then(|m| m.get("licenseName"))
                .and_then(Value::as_str)
                .map(ToString::to_string);

            let mesh_count = json
                .get("meshes")
                .and_then(Value::as_array)
                .map_or(0, Vec::len);

            let expressions = vec![
                "neutral".to_string(),
                "joy".to_string(),
                "angry".to_string(),
                "sorrow".to_string(),
                "fun".to_string(),
            ];

            return (
                CharacterMetadata {
                    title,
                    author,
                    version,
                    license_name,
                    license_url: None,
                    avatar_permission: None,
                    commercial_usage: None,
                    format: ModelFormat::Vrm0,
                    mesh_count,
                },
                expressions,
            );
        }

        // 3. Fallback: Generic glTF 2.0 Binary
        let asset = json.get("asset");
        let title = asset
            .and_then(|a| a.get("generator"))
            .and_then(Value::as_str)
            .unwrap_or("Custom glTF Character")
            .to_string();

        let version = asset
            .and_then(|a| a.get("version"))
            .and_then(Value::as_str)
            .map(ToString::to_string);

        let copyright = asset
            .and_then(|a| a.get("copyright"))
            .and_then(Value::as_str)
            .map(ToString::to_string);

        let mesh_count = json
            .get("meshes")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);

        (
            CharacterMetadata {
                title,
                author: copyright,
                version,
                license_name: Some("glTF 2.0".to_string()),
                license_url: None,
                avatar_permission: None,
                commercial_usage: None,
                format: ModelFormat::Gltf2Binary,
                mesh_count,
            },
            vec!["neutral".to_string(), "focused".to_string()],
        )
    }
}
