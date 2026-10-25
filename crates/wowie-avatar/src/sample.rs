use serde_json::json;

/// Generates a valid, self-contained VRM 1.0 binary model representing "WowieBot 3D".
///
/// Includes actual 3D meshes (head, visor, chassis, thruster, antenna), normals,
/// vertex colors, and full VRMC_vrm avatar metadata.
pub fn create_sample_vrm1_bytes() -> Vec<u8> {
    let mut vertices: Vec<[f32; 3]> = Vec::new();
    let mut normals: Vec<[f32; 3]> = Vec::new();
    let mut colors: Vec<[f32; 4]> = Vec::new();
    let mut indices: Vec<u16> = Vec::new();

    // Helper: Add a 3D box with normals and colors
    let mut add_box = |center: [f32; 3], size: [f32; 3], color: [f32; 4]| {
        let hx = size[0] * 0.5;
        let hy = size[1] * 0.5;
        let hz = size[2] * 0.5;

        let cx = center[0];
        let cy = center[1];
        let cz = center[2];

        // 6 faces * 4 vertices = 24 vertices per box
        let faces = [
            // Front (+Z)
            (
                [0.0, 0.0, 1.0],
                [
                    [cx - hx, cy - hy, cz + hz],
                    [cx + hx, cy - hy, cz + hz],
                    [cx + hx, cy + hy, cz + hz],
                    [cx - hx, cy + hy, cz + hz],
                ],
            ),
            // Back (-Z)
            (
                [0.0, 0.0, -1.0],
                [
                    [cx + hx, cy - hy, cz - hz],
                    [cx - hx, cy - hy, cz - hz],
                    [cx - hx, cy + hy, cz - hz],
                    [cx + hx, cy + hy, cz - hz],
                ],
            ),
            // Top (+Y)
            (
                [0.0, 1.0, 0.0],
                [
                    [cx - hx, cy + hy, cz + hz],
                    [cx + hx, cy + hy, cz + hz],
                    [cx + hx, cy + hy, cz - hz],
                    [cx - hx, cy + hy, cz - hz],
                ],
            ),
            // Bottom (-Y)
            (
                [0.0, -1.0, 0.0],
                [
                    [cx - hx, cy - hy, cz - hz],
                    [cx + hx, cy - hy, cz - hz],
                    [cx + hx, cy - hy, cz + hz],
                    [cx - hx, cy - hy, cz + hz],
                ],
            ),
            // Right (+X)
            (
                [1.0, 0.0, 0.0],
                [
                    [cx + hx, cy - hy, cz + hz],
                    [cx + hx, cy - hy, cz - hz],
                    [cx + hx, cy + hy, cz - hz],
                    [cx + hx, cy + hy, cz + hz],
                ],
            ),
            // Left (-X)
            (
                [-1.0, 0.0, 0.0],
                [
                    [cx - hx, cy - hy, cz - hz],
                    [cx - hx, cy - hy, cz + hz],
                    [cx - hx, cy + hy, cz + hz],
                    [cx - hx, cy + hy, cz - hz],
                ],
            ),
        ];

        for (norm, quad_verts) in faces {
            let base = vertices.len() as u16;
            for pt in quad_verts {
                vertices.push(pt);
                normals.push(norm);
                colors.push(color);
            }
            indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
    };

    // 1. Robot Head (Metallic blue-gray)
    add_box([0.0, 0.45, 0.0], [0.65, 0.55, 0.6], [0.75, 0.82, 0.95, 1.0]);

    // 2. Visor / Face Screen (Deep obsidian / dark cyan)
    add_box(
        [0.0, 0.45, 0.31],
        [0.55, 0.35, 0.05],
        [0.12, 0.22, 0.32, 1.0],
    );

    // 3. Glowing Eyes (Cyan companion eyes)
    add_box(
        [-0.14, 0.46, 0.34],
        [0.12, 0.10, 0.04],
        [0.2, 0.95, 1.0, 1.0],
    );
    add_box(
        [0.14, 0.46, 0.34],
        [0.12, 0.10, 0.04],
        [0.2, 0.95, 1.0, 1.0],
    );

    // 4. Antenna Stem & Glowing Beacon
    add_box([0.0, 0.80, 0.0], [0.06, 0.16, 0.06], [0.6, 0.65, 0.75, 1.0]);
    add_box([0.0, 0.92, 0.0], [0.14, 0.12, 0.14], [1.0, 0.75, 0.2, 1.0]);

    // 5. Neck Ring
    add_box([0.0, 0.14, 0.0], [0.32, 0.08, 0.32], [0.35, 0.4, 0.5, 1.0]);

    // 6. Torso / Chassis
    add_box(
        [0.0, -0.22, 0.0],
        [0.75, 0.65, 0.55],
        [0.85, 0.88, 0.95, 1.0],
    );

    // 7. Core Chest Reactor / Goal Status Core
    add_box(
        [0.0, -0.18, 0.28],
        [0.24, 0.24, 0.06],
        [0.3, 0.85, 0.6, 1.0],
    );

    // 8. Hover Thruster Base
    add_box([0.0, -0.62, 0.0], [0.5, 0.18, 0.45], [0.4, 0.45, 0.55, 1.0]);
    add_box([0.0, -0.74, 0.0], [0.32, 0.1, 0.3], [0.2, 0.7, 1.0, 1.0]);

    // Compute bounding box
    let mut min_pos = vertices[0];
    let mut max_pos = vertices[0];
    for v in &vertices {
        for i in 0..3 {
            min_pos[i] = min_pos[i].min(v[i]);
            max_pos[i] = max_pos[i].max(v[i]);
        }
    }

    // Build binary buffer
    let mut bin_data = Vec::new();

    // 1. Positions: V * 3 * 4 bytes
    let pos_offset = bin_data.len();
    for v in &vertices {
        for val in v {
            bin_data.extend_from_slice(&val.to_le_bytes());
        }
    }
    let pos_len = bin_data.len() - pos_offset;

    // 2. Normals: V * 3 * 4 bytes
    let norm_offset = bin_data.len();
    for n in &normals {
        for val in n {
            bin_data.extend_from_slice(&val.to_le_bytes());
        }
    }
    let norm_len = bin_data.len() - norm_offset;

    // 3. Colors: V * 4 * 4 bytes
    let col_offset = bin_data.len();
    for c in &colors {
        for val in c {
            bin_data.extend_from_slice(&val.to_le_bytes());
        }
    }
    let col_len = bin_data.len() - col_offset;

    // 4. Indices: I * 2 bytes
    let idx_offset = bin_data.len();
    for idx in &indices {
        bin_data.extend_from_slice(&idx.to_le_bytes());
    }
    let idx_len = bin_data.len() - idx_offset;

    // Pad bin_data to 4-byte boundary
    let bin_padding = (4 - (bin_data.len() % 4)) % 4;
    bin_data.extend(std::iter::repeat(0u8).take(bin_padding));
    let padded_bin_len = bin_data.len();

    let v_count = vertices.len();
    let i_count = indices.len();

    // Construct glTF JSON document
    let json_doc = json!({
        "asset": {
            "version": "2.0",
            "generator": "Wowie 3D Companion Studio"
        },
        "scene": 0,
        "scenes": [{ "nodes": [0] }],
        "nodes": [{ "mesh": 0, "name": "WowieBot" }],
        "meshes": [{
            "name": "WowieBotMesh",
            "primitives": [{
                "attributes": {
                    "POSITION": 0,
                    "NORMAL": 1,
                    "COLOR_0": 2
                },
                "indices": 3,
                "mode": 4
            }]
        }],
        "accessors": [
            {
                "bufferView": 0,
                "byteOffset": 0,
                "componentType": 5126, // FLOAT
                "count": v_count,
                "type": "VEC3",
                "min": min_pos,
                "max": max_pos
            },
            {
                "bufferView": 1,
                "byteOffset": 0,
                "componentType": 5126, // FLOAT
                "count": v_count,
                "type": "VEC3"
            },
            {
                "bufferView": 2,
                "byteOffset": 0,
                "componentType": 5126, // FLOAT
                "count": v_count,
                "type": "VEC4"
            },
            {
                "bufferView": 3,
                "byteOffset": 0,
                "componentType": 5123, // UNSIGNED_SHORT
                "count": i_count,
                "type": "SCALAR"
            }
        ],
        "bufferViews": [
            { "buffer": 0, "byteOffset": pos_offset, "byteLength": pos_len, "target": 34962 },
            { "buffer": 0, "byteOffset": norm_offset, "byteLength": norm_len, "target": 34962 },
            { "buffer": 0, "byteOffset": col_offset, "byteLength": col_len, "target": 34962 },
            { "buffer": 0, "byteOffset": idx_offset, "byteLength": idx_len, "target": 34963 }
        ],
        "buffers": [{ "byteLength": padded_bin_len }],
        "extensions": {
            "VRMC_vrm": {
                "specVersion": "1.0",
                "meta": {
                    "name": "WowieBot 3D",
                    "authors": ["Wowie Studio"],
                    "version": "1.0.0",
                    "licenseUrl": "https://vrm.dev/licenses/1.0/",
                    "avatarPermission": "onlyAuthor",
                    "commercialUsage": "personalNonCommercial"
                },
                "expressions": {
                    "preset": {
                        "neutral": {},
                        "happy": {},
                        "relaxed": {},
                        "surprised": {}
                    }
                }
            }
        }
    });

    let json_bytes = serde_json::to_vec(&json_doc).unwrap();
    let json_padding = (4 - (json_bytes.len() % 4)) % 4;
    let padded_json_len = json_bytes.len() + json_padding;

    let total_len = 12 + 8 + padded_json_len + 8 + padded_bin_len;

    let mut glb = Vec::with_capacity(total_len);
    // Header
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2u32.to_le_bytes());
    glb.extend_from_slice(&(total_len as u32).to_le_bytes());

    // Chunk 0: JSON
    glb.extend_from_slice(&(padded_json_len as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend_from_slice(&json_bytes);
    glb.extend(std::iter::repeat(b' ').take(json_padding));

    // Chunk 1: BIN
    glb.extend_from_slice(&(padded_bin_len as u32).to_le_bytes());
    glb.extend_from_slice(b"BIN\0");
    glb.extend_from_slice(&bin_data);

    glb
}
