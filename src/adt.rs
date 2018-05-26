use std::io::{self, Seek, SeekFrom};
use std::sync::Arc;
use std::collections::BTreeMap;
use std::iter::Iterator;
use byteorder::{ReadBytesExt, LE};
use nalgebra::{Vector3, Point3};
use chunked::{Chunked, Chunk};
use reader::{ResourceReader, split_resource_name};
use alpha_map::AlphaMap;
use read_ext::ReadExt;

pub const TILE_SIZE: f32 = 533.0 + 1.0 / 3.0;
pub const CHUNK_SIZE: f32 = TILE_SIZE / 16.0;
pub const UNIT_SIZE: f32 = CHUNK_SIZE / 8.0;
pub const MAP_CENTER: f32 = TILE_SIZE * 32.0; // every map is 64x64 tiles

const MAP_CHUNK_COUNT: usize = 16 * 16;
const MAP_CHUNK_VERTICES: usize = 9 * 9 + 8 * 8;

#[derive(Clone, Debug)]
pub struct MapTile {
    pub textures: Vec<String>,
    pub m2: Vec<String>,
    pub wmo: Vec<String>,
    pub chunks: Vec<MapChunk>,
}

#[derive(Clone, Debug)]
pub struct MapChunk {
    pub index_x: u32,
    pub index_y: u32,
    pub position: Point3<f32>,
    pub heights: Vec<f32>,
    pub normals: Vec<Vector3<f32>>,
    pub holes: Holes,
    pub texture_layers: Vec<TextureLayer>,
}

#[derive(Clone, Debug)]
pub enum Holes {
    LowRes(u16),
    HighRes(u64),
}

#[derive(Clone, Debug)]
pub struct TextureLayer {
    pub texture_id: u32,
    pub flags: TextureLayerFlags,
    pub ground_effect_id: u32,
    pub alpha_map: Option<AlphaMap>,
}

pub struct MapChunkVertices<'a> {
    map_chunk: &'a MapChunk,
    index: usize,
}

pub struct MapChunkVertex {
    pub is_inner: bool,
    pub row: usize,
    pub column: usize,
    pub height: f32,
    pub normal: Vector3<f32>,
}

bitflags! {
    pub struct MapChunkFlags: u32 {
        const HAS_MCSH = 1;
        const IMPASSABLE = 1 << 1;
        const LIQUID_RIVER = 1 << 2;
        const LIQUID_OCEAN = 1 << 3;
        const LIQUID_MAGMA = 1 << 4;
        const LIQUID_SLIME = 1 << 5;
        const HAS_MCCV = 1 << 6;
        const UNKNOWN1 = 1 << 7;
        // skip 7
        const DONT_FIX_ALPHA_MAP = 1 << (7 + 7 + 1);
        const USE_HIGH_RES_HOLES = 1 << (7 + 7 + 2);
    }
}
bitflags! {
    pub struct TextureLayerFlags: u32 {
        const ANIM_ROTATION1 = 1;
        const ANIM_ROTATION2 = 1 << 1;
        const ANIM_ROTATION3 = 1 << 2;
        const ANIM_SPEED1 = 1 << 3;
        const ANIM_SPEED2 = 1 << 4;
        const ANIM_SPEED3 = 1 << 5;
        const ANIM_ENABLED = 1 << 6;
        const OVERBRIGHT = 1 << 7;
        const USE_ALPHA_MAP = 1 << 8;
        const ALPHA_MAP_COMPRESSED = 1 << 9;
        const USE_CUBE_MAP_REFLECTION = 1 << 10;
        const UNKNOWN1 = 1 << 11;
        const UNKNOWN2 = 1 << 12;
    }
}
impl_bitflags_from_read!(read_u32, MapChunkFlags);
impl_bitflags_from_read!(read_u32, TextureLayerFlags);

impl MapTile {
    pub fn load(reader: Arc<ResourceReader>, name: &str, big_alpha: Option<bool>) -> io::Result<MapTile> {
        // vector of (split_name, is_rootfile)
        let mut targets = vec![(name.to_owned(), true)];

        // add split files (cata+)
        let (name_dir, name_file, _) = split_resource_name(name);
        for split_id in ["tex", "obj"].iter() {
            let split_name = format!("{}{}_{}0.adt", name_dir, name_file, split_id);
            if reader.exists(&split_name)? { targets.push((split_name, false)) }
        }

        let map_chunks = vec![MapChunk {
            index_x: 0,
            index_y: 0,
            position: Point3::new(0.0, 0.0, 0.0),
            heights: Vec::with_capacity(MAP_CHUNK_VERTICES),
            normals: Vec::with_capacity(MAP_CHUNK_VERTICES),
            holes: Holes::LowRes(0),
            texture_layers: Vec::with_capacity(4),
        }; MAP_CHUNK_COUNT];

        let mut map_tile = MapTile {
            textures: Vec::new(),
            m2: Vec::new(),
            wmo: Vec::new(),
            chunks: map_chunks,
        };

        for &(ref split_name, is_rootfile) in targets.iter() {
            let mut input = reader.open(&split_name)?;
            read_adt_file(&mut map_tile, &mut input, is_rootfile, big_alpha)?;
        }

        Ok(map_tile)
    }
}

impl MapChunk {
    pub fn each_triangle<F>(high_detail: bool, mut f: F) where F: FnMut((u16, u16, u16)) {
        for row in 0..8 as u16 {
            let row_offset = row * (9 + 8);

            for column in 0..8 as u16 {
                let topleft = row_offset + column;
                let topright = topleft + 1;
                let center = row_offset + 9 + column;
                let botleft = row_offset + 9 + 8 + column;
                let botright = botleft + 1;

                if high_detail {
                    f((topleft, topright, center));
                    f((topright, botright, center));
                    f((botright, botleft, center));
                    f((botleft, topleft, center));
                } else {
                    f((topleft, topright, botleft));
                    f((topright, botright, botleft));
                }
            }
        }
    }
}

impl<'a> MapChunkVertices<'a> {
    pub fn new(map_chunk: &'a MapChunk) -> Self {
        MapChunkVertices { map_chunk, index: 0 }
    }
}

impl<'a> Iterator for MapChunkVertices<'a> {
    type Item = MapChunkVertex;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(height) = self.map_chunk.heights.get(self.index) {
            let height = *height;
            let normal = self
                .map_chunk
                .normals
                .get(self.index)
                .map(|n| n.clone())
                .unwrap_or_else(|| Vector3::new(0.0, 0.0, 1.0));

            let square_row_offset = self.index % (9 + 8);
            let is_inner = square_row_offset < 9;
            let column = if is_inner { square_row_offset - 9 } else { square_row_offset };
            let row = self.index / (9 + 8);

            self.index += 1;

            Some(MapChunkVertex {
                is_inner,
                row,
                column,
                height,
                normal,
            })
        } else {
            None
        }
    }
}

fn read_adt_file<R: ReadExt>(map_tile: &mut MapTile, input: &mut R, is_rootfile: bool, big_alpha: Option<bool>) -> io::Result<()> {
    let mut m2_tmp = BTreeMap::new();
    let mut wmo_tmp = BTreeMap::new();

    let mut map_chunk_index = 0;

    for chunk in Chunked::new(input) {
        let chunk = chunk?;
        let mut cursor = chunk.cursor();

        match chunk.token.as_str() {
            // texture paths
            "MTEX" => {
                cursor.read_cstring_array_into(&mut map_tile.textures)?;
            }
            // m2 paths
            "MMDX" => {
                cursor.read_cstring_table_into(&mut m2_tmp)?;
            }
            // m2 local ids
            "MMID" => {
                read_ids_chunk(&chunk, &mut m2_tmp, &mut map_tile.m2)?;
            }
            // wmo paths
            "MWMO" => {
                cursor.read_cstring_table_into(&mut wmo_tmp)?;
            }
            // wmo local ids
            "MWID" => {
                read_ids_chunk(&chunk, &mut wmo_tmp, &mut map_tile.wmo)?;
            }
            // m2 placement definitions
            "MDDF" => {
                for _ in 0..(chunk.data.len() / 36) {
                    let_read! { LE | cursor =>
                        name_id: u32;
                        unique_id: u32;
                        position: (f32, f32, f32);
                        rotation: (f32, f32, f32);
                        scale: u16;
                        flags: u16;
                    }
                }
            }
            // wmo placement definitions
            "MODF" => {
                for _ in 0..(chunk.data.len() / 64) {
                    let_read! { LE | cursor =>
                        name_id: u32;
                        unique_id: u32;
                        position: (f32, f32, f32);
                        rotation: (f32, f32, f32);
                        aabox_min: (f32, f32, f32);
                        aabox_max: (f32, f32, f32);
                        flags: u16;
                        doodad_set: u16;
                        name_set: u16;
                        unknown1: u16;
                    }
                }
            }
            // map chunk
            "MCNK" => {
                let map_chunk = &mut map_tile.chunks[map_chunk_index];
                if is_rootfile { read_mcnk_header(map_chunk, &mut cursor)? }
                read_mcnk_subchunks(map_chunk, &mut cursor, big_alpha)?;
                map_chunk_index += 1;
            }
            _ => {}
        }
    }

    Ok(())
}

fn read_mcnk_header<R: ReadExt>(map_chunk: &mut MapChunk, mut cursor: &mut R) -> io::Result<()> {
    // TODO: add 3.3.5 compatibility ifs
    let_read! { LE | cursor =>
        flags: MapChunkFlags;
        index_x: u32;
        index_y: u32;
        num_layers: u32;
        num_doodad_refs: u32;
        holes_high_res: u64;
        offset_layer: u32;
        offset_refs: u32;
        offset_alpha: u32;
        size_alpha: u32;
        offset_shadow: u32;
        size_shadow: u32;
        area_id: u32;
        num_mapobj_refs: u32;
        holes_low_res: u16;
        unknown1: u16;
        texmap1: u64;
        texmap2: u64;
        no_effect_doodad: u64;
        offset_sound_emitters: u32;
        num_sound_emitters: u32;
        offset_liquid: u32;
        size_liquid: u32;
        position: (f32, f32, f32);
        offset_mccv: u32;
        offset_mclv: u32;
        unknown2: u32;
    }

    map_chunk.index_x = index_x;
    map_chunk.index_y = index_y;

    // FIXME: this is Y-up coordinate; must be converted to Z-up
    map_chunk.position = Point3::new(position.0, position.1, position.2);

    // FIXME: high-res holes might use different algorithm (see wowdev wiki)
    map_chunk.holes = if flags.contains(MapChunkFlags::USE_HIGH_RES_HOLES) {
        Holes::HighRes(holes_high_res)
    } else {
        Holes::LowRes(holes_low_res)
    };

    Ok(())
}

fn read_mcnk_subchunks<R: ReadExt>(map_chunk: &mut MapChunk, cursor: &mut R, big_alpha: Option<bool>) -> io::Result<()> {
    let mut mcal_offsets = Vec::new();

    for subchunk in Chunked::new(cursor) {
        let subchunk = subchunk?;
        let mut subcursor = subchunk.cursor();

        match subchunk.token.as_str() {
            // heights
            "MCVT" => {
                for _ in 0..MAP_CHUNK_VERTICES {
                    let z = subcursor.read_f32::<LE>()?;
                    map_chunk.heights.push(z);
                }
            }
            // normals
            "MCNR" => {
                for _ in 0..MAP_CHUNK_VERTICES {
                    let x = subcursor.read_i8()? as f32 / -127f32;
                    let y = subcursor.read_i8()? as f32 / -127f32;
                    let z = subcursor.read_i8()? as f32 / 127f32;

                    map_chunk.normals.push(Vector3::new(x, y, z).normalize());
                }
            }
            // textures
            "MCLY" => {
                for _ in 0..(subchunk.data.len() / 16) {
                    let_read! { LE | subcursor =>
                        texture_id: u32;
                        flags: TextureLayerFlags;
                        mcal_offset: u32;
                        ground_effect_id: u32;
                    }

                    map_chunk.texture_layers.push(TextureLayer {
                        texture_id,
                        flags,
                        ground_effect_id,
                        alpha_map: None
                    });
                    mcal_offsets.push(mcal_offset as usize);
                }
            }
            // alpha maps
            "MCAL" => {
                for (index, start) in mcal_offsets.iter().enumerate() {
                    let layer = &mut map_chunk.texture_layers[index];

                    if !layer.flags.contains(TextureLayerFlags::USE_ALPHA_MAP) {
                        continue;
                    }

                    subcursor.seek(SeekFrom::Start(*start as u64))?;

                    layer.alpha_map = if layer.flags.contains(TextureLayerFlags::ALPHA_MAP_COMPRESSED) {
                        Some(AlphaMap::read_compressed(&mut subcursor)?)
                    } else if let Some(is_u8) = big_alpha {
                        Some(AlphaMap::read_raw(&mut subcursor, !is_u8)?)
                    } else {
                        warn!("Skipping non-compressed alpha map {} (missing big_alpha option)", index);
                        None
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

fn read_ids_chunk(chunk: &Chunk, data: &mut BTreeMap<u32, String>, into: &mut Vec<String>) -> io::Result<()> {
    into.reserve_exact(data.len());

    let mut cursor = chunk.cursor();
    for _ in 0..(chunk.data.len() / 4) {
        let offset = cursor.read_u32::<LE>()?;

        if let Some(name) = data.remove(&offset) {
            into.push(name)
        } else {
            warn!("found invalid offset {}", offset);
        }
    }

    for (_, name) in data.iter() {
        warn!("missing reference for {}", &name);
    }

    data.clear();
    Ok(())
}
