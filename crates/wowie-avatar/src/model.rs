use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CharacterId(Uuid);

impl CharacterId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }

    pub fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }

    pub fn into_uuid(self) -> Uuid {
        self.0
    }
}

impl Default for CharacterId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for CharacterId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for CharacterId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelFormat {
    Vrm1,
    Vrm0,
    Gltf2Binary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterMetadata {
    pub title: String,
    pub author: Option<String>,
    pub version: Option<String>,
    pub license_name: Option<String>,
    pub license_url: Option<String>,
    pub avatar_permission: Option<String>,
    pub commercial_usage: Option<String>,
    pub format: ModelFormat,
    pub mesh_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CharacterProfile {
    pub id: CharacterId,
    pub display_name: String,
    pub file_name: String,
    pub file_size_bytes: u64,
    pub metadata: CharacterMetadata,
    pub available_expressions: Vec<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompanionMood {
    Neutral,
    Focused,
    Concerned,
    Relaxed,
    Celebrating,
    Thinking,
    Snoozed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompanionEvent {
    Idle,
    FocusStarted,
    DistractionDetected,
    BreakStarted,
    WorkResumed,
    GoalCompleted,
    SnoozeStarted,
}

impl CompanionEvent {
    pub fn to_mood(&self) -> CompanionMood {
        match self {
            Self::Idle => CompanionMood::Neutral,
            Self::FocusStarted => CompanionMood::Focused,
            Self::DistractionDetected => CompanionMood::Concerned,
            Self::BreakStarted => CompanionMood::Relaxed,
            Self::WorkResumed => CompanionMood::Focused,
            Self::GoalCompleted => CompanionMood::Celebrating,
            Self::SnoozeStarted => CompanionMood::Snoozed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Vertex3D {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 4],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Mesh3D {
    pub name: String,
    pub vertices: Vec<Vertex3D>,
    pub indices: Vec<u32>,
    #[serde(default)]
    pub double_sided: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox3D {
    pub min: [f32; 3],
    pub max: [f32; 3],
    pub center: [f32; 3],
    pub size: [f32; 3],
    pub max_extent: f32,
}

impl BoundingBox3D {
    pub fn from_vertices(vertices: &[Vertex3D]) -> Self {
        if vertices.is_empty() {
            return Self {
                min: [0.0, 0.0, 0.0],
                max: [0.0, 0.0, 0.0],
                center: [0.0, 0.0, 0.0],
                size: [0.0, 0.0, 0.0],
                max_extent: 1.0,
            };
        }

        let mut min = vertices[0].position;
        let mut max = vertices[0].position;

        for v in vertices {
            for i in 0..3 {
                if v.position[i] < min[i] {
                    min[i] = v.position[i];
                }
                if v.position[i] > max[i] {
                    max[i] = v.position[i];
                }
            }
        }

        let center = [
            (min[0] + max[0]) * 0.5,
            (min[1] + max[1]) * 0.5,
            (min[2] + max[2]) * 0.5,
        ];

        let size = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
        let max_extent = size[0].max(size[1]).max(size[2]).max(0.001);

        Self {
            min,
            max,
            center,
            size,
            max_extent,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Model3D {
    pub metadata: CharacterMetadata,
    pub available_expressions: Vec<String>,
    pub meshes: Vec<Mesh3D>,
    pub bounds: BoundingBox3D,
    pub total_vertex_count: usize,
    pub total_triangle_count: usize,
}

impl Model3D {
    pub fn is_empty(&self) -> bool {
        self.meshes.is_empty() || self.total_vertex_count == 0
    }

    /// Center the model at the origin and scale it so its maximum dimension is normalized to target_size.
    pub fn normalize_and_center(&mut self, target_size: f32) {
        if self.is_empty() {
            return;
        }

        let scale = target_size / self.bounds.max_extent;
        let center = self.bounds.center;

        for mesh in &mut self.meshes {
            for v in &mut mesh.vertices {
                v.position[0] = (v.position[0] - center[0]) * scale;
                v.position[1] = (v.position[1] - center[1]) * scale;
                v.position[2] = (v.position[2] - center[2]) * scale;
            }
        }

        // Recompute bounds after normalization
        let all_vertices: Vec<Vertex3D> = self
            .meshes
            .iter()
            .flat_map(|m| m.vertices.clone())
            .collect();
        self.bounds = BoundingBox3D::from_vertices(&all_vertices);
    }
}
