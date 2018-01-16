use std::mem::drop;
use std::collections::BTreeMap;
use std::io::{self, Cursor};
use std::sync::Arc;
use byteorder::{ReadBytesExt, WriteBytesExt, LittleEndian as LE};
use read_ext::ReadExt;
use chunked_old::Chunked;
use reader::{ResourceReader, split_resource_name};

#[allow(dead_code)]
struct Material {

}

#[allow(dead_code)]
pub struct RootFile {
    num_groups: u32,
    texture_table: BTreeMap<u32, String>,
}

#[allow(dead_code)]
struct GroupFile {

}

#[allow(dead_code)]
pub fn load(reader: Arc<ResourceReader>, name: &str) -> io::Result<RootFile> {
    let chunks = Chunked::read(&mut reader.open(name)?)?;

    let_chunk! { LE | chunks "MVER" =>
        version: u32;
    }
    if version != 17 {
        return Err(io_error!(InvalidData, "unsupported WMO version: {}", version))
    }

    // header
    let_chunk! { LE | chunks "MOHD" =>
        num_textures: u32;
        num_groups: u32;
        num_portals: u32;
        num_lights: u32;
        num_doodad_names: u32;
        num_doodad_defs: u32;
        num_doodad_sets: u32;
        ambient_color: (u8, u8, u8, u8);
        wmo_id: u32;
        bounding_box_min: (f32, f32, f32);
        bounding_box_min: (f32, f32, f32);
        flags: u16;
        num_lod: u16;
    }

    // texture list
    let texture_table = chunks.cursor("MOTX")?.read_cstring_table()?;

    // materials
    let mut momt = chunks.cursor("MOMT")?;
    for _ in 0..num_textures {
        trace!("--- NEW MATERIAL ---");
        let_read! { LE | momt =>
            flags: i32;
            specular_mode: i32;
            transparent: i32; // Blending: 0 for opaque, 1 for transparent
            name_start: i32; // Start position for the first texture filename in the MOTX data block
            color1: (u8, u8, u8, u8);
            flag1: u32;
            name_end: i32; // Start position for the second texture filename in the MOTX data block
            color2: (u8, u8, u8, u8);
            flag2: u32;
            f1: f32;
            f2: f32;
            dx1: i32;
            dx2: i32;
            dx3: i32;
            dx4: i32;
            dx5: i32;
        }

        if name_start >= 0 {
            let texture_key = texture_table.get(&(name_start as u32));
            trace!("name_start = {:?}", texture_key);
        }
        if name_end >= 0 {
            let texture_key = texture_table.get(&(name_end as u32));
            trace!("name_end = {:?}", texture_key);
        }
    }
    drop(momt);

    // group names
    let _texture_table = chunks.cursor("MOGN")?.read_cstring_table()?;

    // group information "MOGI"
    // lighting "MOLT"
    // doodad sets "MODS"

    // doodad names "MODN"
    let _doodad_names_table = chunks.cursor("MODN")?.read_cstring_table()?;

    // doodad instances "MODD"

    // TODO: group file load

    for group_index in 0..num_groups {
        let group_path = {
            let (dir, file, _) = split_resource_name(name);
            format!("{}{}_{:03}.wmo", dir, file, group_index)
        };

        let mut group_input = Chunked::read(&mut reader.open(&group_path)?)?;
        let mut group_data = group_input.cursor("MOGP")?;
        let_read! { LE | group_data =>
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

        let inner_chunks = Chunked::read(&mut group_data)?;

        let mut index_data = Vec::<u16>::new();
        let mut vertex_data = Cursor::new(Vec::<u8>::new());

        let mut movi = inner_chunks.cursor("MOVI")?;
        for _ in 0..(movi.get_ref().len() / 2) {
            let i = movi.read_u16::<LE>()?;
            index_data.push(i);
        }

        let mut movt = inner_chunks.cursor("MOVT")?;
        let mut monr = inner_chunks.cursor("MONR")?;
        let mut motv = inner_chunks.cursor("MOTV")?;

        for _ in 0..(movt.get_ref().len() / (3 * 4)) {
            let px = movt.read_f32::<LE>()?;
            let py = movt.read_f32::<LE>()?;
            let pz = movt.read_f32::<LE>()?;

            let nx = monr.read_f32::<LE>()?;
            let ny = monr.read_f32::<LE>()?;
            let nz = monr.read_f32::<LE>()?;

            let tx = motv.read_f32::<LE>()?;
            let ty = motv.read_f32::<LE>()?;

            vertex_data.write_f32::<LE>(px)?;
            vertex_data.write_f32::<LE>(pz)?;
            vertex_data.write_f32::<LE>(-py)?;

            vertex_data.write_f32::<LE>(nx)?;
            vertex_data.write_f32::<LE>(nz)?;
            vertex_data.write_f32::<LE>(-ny)?;

            vertex_data.write_f32::<LE>(tx)?;
            vertex_data.write_f32::<LE>(ty)?;
        }

        let mut moba = inner_chunks.cursor("MOBA")?;
        for _ in 0..(moba.get_ref().len() / 24) {
            trace!("--- NEW RENDER BATCH ---");
            let_read! { LE | moba =>
                unknown1: u16;
                unknown2: u16;
                unknown3: u16;
                unknown4: u16;
                unknown5: u16;
                unknown6: u16;
                index_start: u32;
                index_count: u16;
                vertex_start: u16;
                vertex_end: u16;
                flags: u8;
                material_id: u8;
            }
        }
    }

    Ok(RootFile {
        num_groups: 0,
        texture_table: BTreeMap::new()
    })
}
