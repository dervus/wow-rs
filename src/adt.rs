use std::io;
use std::sync::Arc;
use byteorder::{ReadBytesExt, LittleEndian as LE};
use cgmath::prelude::*;
use cgmath::Vector3;
use read_ext::ReadExt;
use chunked::Chunked;
use reader::ResourceReader;

const TILE_SIZE: f32 = 533.0 + 1.0 / 3.0;
const CHUNK_SIZE: f32 = TILE_SIZE / 16.0;
const UNIT_SIZE: f32 = CHUNK_SIZE / 8.0;
#[allow(dead_code)]
const MAP_CENTER: f32 = TILE_SIZE * 32.0;

#[allow(dead_code)]
pub struct HeightMapVertex {
    pub position: (f32, f32, f32),
    pub normal: (f32, f32, f32),
    pub texcoord: (f32, f32),
}

#[allow(dead_code)]
pub struct MapTile {
    pub vertices: Vec<HeightMapVertex>,
    pub faces: Vec<(u32, u32, u32)>,
}

// Kalimdor_40_38.adt chunk dump:
// MVER (4 bytes)
// MHDR (64 bytes)
// MH2O (28576 bytes)
// MCNK (x256, 1180 bytes each)
// MFBO (36 bytes)

#[allow(dead_code)]
pub fn load(reader: Arc<ResourceReader>, name: &str) -> io::Result<MapTile> {
    let mut input = reader.open(name)?;
    let chunks = Chunked::read(&mut input)?;

    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    let mut vertex_index = 0u32;

    for mut data in chunks.cursor_all("MCNK")? {
        // TODO: add high-res holes data support
        let_read! { data =>
            u32 flags;
            u32 index_x;
            u32 index_y;
            u32 num_layers;
            u32 num_doodad_refs;
            u32 offset_height;
            u32 offset_normal;
            u32 offset_layer;
            u32 offset_refs;
            u32 offset_alpha;
            u32 size_alpha;
            u32 offset_shadow;
            u32 size_shadow;
            u32 area_id;
            u32 num_mapobj_refs;
            u16 holes;
            u16 unknown1;
            u64 texmap1;
            u64 texmap2;
            u64 no_effect_doodad;
            u32 offset_sound_emitters;
            u32 num_sound_emitters;
            u32 offset_liquid;
            u32 size_liquid;
            f32tuple3 position;
            u32 offset_mccv;
            u32 offset_mclv;
            u32 unused;
        }

        let subchunks = Chunked::read(&mut data)?;

        let mut mcvt = subchunks.cursor("MCVT")?;
        let mut mcnr = subchunks.cursor("MCNR")?;

        for row in 0..17 {
            let is_inner = row % 2 != 0;
            let num_columns = if is_inner { 8 } else { 9 };
            for column in 0..num_columns {
                let height = mcvt.read_f32::<LE>()?;

                let mut x = column as f32 * UNIT_SIZE;
                if is_inner { x += 0.5f32 * UNIT_SIZE }
                x += UNIT_SIZE * 8.0f32 * index_x as f32;
                let y = row as f32 * UNIT_SIZE * 0.5f32 + UNIT_SIZE * 8.0f32 * index_y as f32;
                let z = position.2 + height;

                let nx = mcnr.read_i8()? as f32 / -127f32;
                let ny = mcnr.read_i8()? as f32 / -127f32;
                let nz = mcnr.read_i8()? as f32 / 127f32;

                let position = (x, z, -y);
                let normal = Vector3::new(nx, nz, -ny).normalize();

                vertices.push(HeightMapVertex {
                    position,
                    normal: (normal.x, normal.y, normal.z),
                    texcoord: (0f32, 0f32)
                });
            }
        }

        for row in 0..8 {
            let row_offset = row * (9 + 8);
            for column in 0..8 {
                let topleft  = vertex_index + row_offset + column;
                let topright = topleft + 1;
                let center   = vertex_index + row_offset + 9 + column;
                let botleft  = vertex_index + row_offset + 9 + 8 + column;
                let botright = botleft + 1;

                faces.push((topleft, topright, center));
                faces.push((topright, botright, center));
                faces.push((botright, botleft, center));
                faces.push((botleft, topleft, center));
            }
        }

        vertex_index += 9 * 9 + 8 * 8;
    }

    Ok(MapTile { vertices, faces })
}