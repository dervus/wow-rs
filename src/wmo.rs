use std::iter::Iterator;
use std::collections::BTreeMap;
use std::io::{self, Seek, SeekFrom};
use std::sync::Arc;
use byteorder::{ReadBytesExt, LittleEndian as LE};
use read_ext::ReadExt;
use chunked::Chunked;
use reader::{ResourceReader, split_resource_name};
use misc::Blob;

#[derive(Debug)]
pub struct MapObject {
    pub textures: Vec<String>,
    pub m2: Vec<String>,
    pub materials: Vec<Material>,
    pub groups: Vec<MeshGroupInfo>,
}

#[derive(Debug)]
pub struct Material {
    pub texture_id: Option<usize>,
}

#[derive(Debug)]
pub struct MeshGroupInfo {
    pub resource_key: String,
    pub flags: u32,
    pub bounding_box_min: (f32, f32, f32),
    pub bounding_box_max: (f32, f32, f32),
    pub name: Option<String>,
}

#[derive(Debug)]
pub struct MeshGroup {
    pub indexes: Blob<u16>,
    pub vertices: Blob<(f32, f32, f32)>,
    pub normals: Blob<(f32, f32, f32)>,
    pub texcoords: Blob<(f32, f32)>,
    pub batches: Vec<RenderBatch>,
}

#[derive(Debug)]
pub struct RenderBatch {
    pub material_id: u16,
    pub index_start: u32,
    pub index_count: u16,
    pub vertex_start: u16,
    pub vertex_end: u16,
}

impl MapObject {
    pub fn load_all_groups(&self, reader: Arc<ResourceReader>) -> Vec<MeshGroup> {
        self.groups.iter().flat_map(|group| {
            match group.load(reader.clone()) {
                Ok(result) => Some(result),
                Err(error) => {
                    error!("Unable to load WMO mesh group: {}", error);
                    None
                }
            }
        }).collect()
    }
}

impl MeshGroupInfo {
    pub fn load(&self, reader: Arc<ResourceReader>) -> io::Result<MeshGroup> {
        let mut input = reader.open(&self.resource_key)?;
        read_group_file(&mut input)
    }
}

pub fn load(reader: Arc<ResourceReader>, name: &str) -> io::Result<MapObject> {
    let input = reader.open(&name)?;

    let mut map_object = MapObject {
        textures: Vec::new(),
        m2: Vec::new(),
        materials: Vec::new(),
        groups: Vec::new()
    };

    let mut textures_index = BTreeMap::new();
    let mut group_names_table = BTreeMap::new();
    let mut m2_table = BTreeMap::new();

    for chunk in Chunked::new(input) {
        let chunk = chunk?;
        let mut cursor = chunk.cursor();

        match chunk.token.as_str() {
            // version
            "MVER" => {
                let version = cursor.read_u32::<LE>()?;
                if version != 17 {
                    return Err(io_error!(InvalidData, "unsupported WMO version: {}", version))
                }
            }
            // header
            "MOHD" => {
                let_read! { LE | cursor =>
                    num_materials: u32;
                    num_groups: u32;
                    num_portals: u32;
                    num_lights: u32;
                    num_doodad_names: u32;
                    num_doodad_defs: u32;
                    num_doodad_sets: u32;
                    ambient_color: (u8, u8, u8, u8);
                    wmo_id: u32;
                    bounding_box_min: (f32, f32, f32);
                    bounding_box_max: (f32, f32, f32);
                    flags: u16;
                    num_lod: u16;
                }
            }
            // texture paths
            "MOTX" => {
                cursor.read_cstring_table_with(|offset, path| {
                    let index = map_object.textures.len();
                    map_object.textures.push(path);
                    textures_index.insert(offset, index);
                })?;
            }
            // materials
            "MOMT" => {
                for index in 0..(chunk.data.len() / 64) {
                    trace!("--- MATERIAL #{} ---", index);

                    // WoWModelViewer struct
                    // let_read! { LE | cursor =>
                    //     flags: i32;
                    //     specular_mode: i32;
                    //     transparent: i32; // Blending: 0 for opaque, 1 for transparent
                    //     name_start: i32; // Start position for the first texture filename in the MOTX data block
                    //     color1: (u8, u8, u8, u8);
                    //     flag1: u32;
                    //     name_end: i32; // Start position for the second texture filename in the MOTX data block
                    //     color2: (u8, u8, u8, u8);
                    //     flag2: u32;
                    //     f1: f32;
                    //     f2: f32;
                    //     dx1: i32;
                    //     dx2: i32;
                    //     dx3: i32;
                    //     dx4: i32;
                    //     dx5: i32;
                    // }

                    cursor.seek(SeekFrom::Start((index * 64) as u64))?;
                    let_read! { LE | cursor =>
                        flags: u32;
                        shader_id: u32;
                        blend_mod: u32;
                        diffuse_name_index: u32;
                        emissive_color: (u8, u8, u8, u8);
                        sidn_emissive_color: (u8, u8, u8, u8); // self-illuminated d? n?
                        env_name_index: u32;
                        diff_color: u32;
                        ground_type: u32;
                        texture_2: u32;
                        color_2: u32;
                        flags_2: u32;
                    }

                    // if name_start >= 0 {
                    //     let texture_key = textures_index.get(&(name_start as u32));
                    //     trace!("name_start = {:?}", texture_key);
                    // }
                    // if name_end >= 0 {
                    //     let texture_key = textures_index.get(&(name_end as u32));
                    //     trace!("name_end = {:?}", texture_key);
                    // }
                    //
                    // let texture_id = if name_start >= 0 {
                    //     textures_index.get(&(name_start as u32)).map(|id| *id)
                    // } else {
                    //     None
                    // };
                    //
                    // map_object.materials.push(Material {
                    //     texture_id
                    // });
                }
            }
            // wmo group names
            "MOGN" => {
                cursor.read_cstring_table_into(&mut group_names_table)?;
            }
            // wmo group metadata
            "MOGI" => {
                for index in 0..(chunk.data.len() / 32) {
                    trace!("--- MESH GROUP INFO #{} ---", index);
                    let_read! { LE | cursor =>
                        flags: u32;
                        bounding_box_min: (f32, f32, f32);
                        bounding_box_max: (f32, f32, f32);
                        name_offset: i32;
                    }

                    let group_name = if name_offset >= 0 {
                        group_names_table.get(&(name_offset as u32)).map(|s| s.to_owned())
                    } else {
                        None
                    };

                    let group_path = {
                        let (dir, file, _) = split_resource_name(name);
                        format!("{}{}_{:03}.wmo", dir, file, index)
                    };

                    let group_info = MeshGroupInfo {
                        resource_key: group_path,
                        flags,
                        bounding_box_min,
                        bounding_box_max,
                        name: group_name
                    };

                    map_object.groups.push(group_info);
                }
            }
            // doodad sets
            "MODS" => {
            }
            // m2 paths
            "MODN" => {
                cursor.read_cstring_table_into(&mut m2_table)?;
            }
            // m2 placement definitions
            "MODD" => {
            }
            _ => {}
        }
    }

    Ok(map_object)
}

fn read_group_file<R: ReadExt>(input: &mut R) -> io::Result<MeshGroup> {
    let mut mesh_group = MeshGroup {
        indexes: Vec::new().into(),
        vertices: Vec::new().into(),
        normals: Vec::new().into(),
        texcoords: Vec::new().into(),
        batches: Vec::new()
    };

    for chunk in Chunked::new(input) {
        let root_chunk = chunk?;
        let mut root_cursor = root_chunk.cursor();

        match root_chunk.token.as_str() {
            // version
            "MVER" => {
                let version = root_cursor.read_u32::<LE>()?;
                if version != 17 {
                    return Err(io_error!(InvalidData, "unsupported WMO version: {}", version))
                }
            }
            "MOGP" => {
                let_read! { LE | root_cursor =>
                    group_id: u32;
                    group_name: u32;
                    group_flags1: u32;
                    group_aabox_min: (f32, f32, f32);
                    group_aabox_max: (f32, f32, f32);
                    portal_start: u16;
                    portal_count: u16;
                    trans_batch_count: u16;
                    int_batch_count: u16;
                    ext_batch_count: u16;
                    padding_or_batch_type: u16;
                    fog_id1: u8;
                    fog_id2: u8;
                    fog_id3: u8;
                    fog_id4: u8;
                    group_liquid: u32;
                    unique_id: u32;
                    group_flags2: u32;
                    unknown: u32;
                }

                for chunk in Chunked::new(root_cursor) {
                    let chunk = chunk?;
                    let mut cursor = chunk.cursor();

                    match chunk.token.as_str() {
                        "MOVI" => {
                            for _ in 0..(chunk.data.len() / 2) {
                                let index = cursor.read_u16::<LE>()?;
                                mesh_group.indexes.push(index)
                            }
                        }
                        "MOVT" => {
                            for _ in 0..(chunk.data.len() / (3 * 4)) {
                                let position = cursor.read_f32tuple3::<LE>()?;
                                mesh_group.vertices.push(position);
                            }
                        }
                        "MONR" => {
                            for _ in 0..(chunk.data.len() / (3 * 4)) {
                                let normal = cursor.read_f32tuple3::<LE>()?;
                                mesh_group.normals.push(normal);
                            }
                        }
                        "MOTV" => {
                            for _ in 0..(chunk.data.len() / (2 * 4)) {
                                let texcoord = cursor.read_f32tuple2::<LE>()?;
                                mesh_group.texcoords.push(texcoord);
                            }
                        }
                        "MOBA" => {
                            for index in 0..(chunk.data.len() / 24) {
                                trace!("--- RENDER BATCH #{} ---", index);
                                let_read! { LE | cursor =>
                                    unknown1: u16;
                                    unknown2: u16;
                                    unknown3: u16;
                                    unknown4: u16;
                                    unknown5: u16;
                                    material_id_large: u16;
                                    index_start: u32;
                                    index_count: u16;
                                    vertex_start: u16;
                                    vertex_end: u16;
                                    flags: u8;
                                    material_id_little: u8;
                                }

                                let render_batch = RenderBatch {
                                    material_id: material_id_little as u16,
                                    index_start,
                                    index_count,
                                    vertex_start,
                                    vertex_end
                                };

                                mesh_group.batches.push(render_batch);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    Ok(mesh_group)
}
