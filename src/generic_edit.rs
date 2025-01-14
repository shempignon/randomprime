use crate::{
    door_meta::DoorType,
    patcher::PatcherState,
    mlvl_wrapper,
};

use reader_writer::CStrConversionExtension;

use std::collections::HashMap;

use crate::patch_config::EditObjConfig;

pub fn patch_edit_objects<'r>
(
    _ps: &mut PatcherState,
    area: &mut mlvl_wrapper::MlvlArea<'r, '_, '_, '_>,
    edit_objs: HashMap<u32, EditObjConfig>,
)
-> Result<(), String>
{
    let mrea_id = area.mlvl_area.mrea.to_u32().clone();

    /* Add layers */
    for (_, config) in edit_objs.iter() {
        if config.layer.is_none() {
            continue;
        }

        let layer_id = config.layer.unwrap();
        if layer_id >= 63 {
            panic!("Layer #{} above maximum (63) in room 0x{:X}", layer_id, mrea_id);
        }

        while area.layer_flags.layer_count <= layer_id {
            let mrea_id = area.mlvl_area.mrea.to_u32().clone();
            area.add_layer(b"New Layer\0".as_cstr());
            if area.layer_flags.layer_count >= 64 {
                panic!("Ran out of layers in room 0x{:X}", mrea_id);
            }
        }
    }

    let scly = area.mrea().scly_section_mut();

    /* Move Objects */

    for (obj_id, config) in edit_objs.iter() {
        if config.layer.is_none() {
            continue;
        }

        let obj_id = obj_id & 0x00FFFFFF;
        let layer_id = config.layer.unwrap() as usize;

        // find existing object
        let old_layer_id = {
            let mut info = None;

            let layer_count = scly.layers.as_mut_vec().len();
            for _layer_id in 0..layer_count {
                let layer = scly.layers
                    .iter()
                    .nth(_layer_id)
                    .unwrap();

                let obj = layer.objects
                    .iter()
                    .find(|obj| obj.instance_id & 0x00FFFFFF == obj_id);

                if let Some(obj) = obj {
                    info = Some((_layer_id as u32, obj.instance_id));
                    break;
                }
            }

            let (old_layer_id, _) = info.expect(format!("Cannot find object 0x{:X} in room 0x{:X}", obj_id, mrea_id).as_str());

            old_layer_id
        };

        // clone existing object
        let obj = scly.layers
            .as_mut_vec()[old_layer_id as usize]
            .objects
            .as_mut_vec()
            .iter_mut()
            .find(|obj| obj.instance_id & 0x00FFFFFF == obj_id)
            .unwrap()
            .clone();

        // remove original
        scly.layers
            .as_mut_vec()[old_layer_id as usize]
            .objects
            .as_mut_vec()
            .retain(|obj| obj.instance_id & 0x00FFFFFF != obj_id);

        // re-add to target layer
        scly.layers
            .as_mut_vec()[layer_id as usize]
            .objects
            .as_mut_vec()
            .push(obj);
    }

    /* Edit Properties */
    
    let scly = area.mrea().scly_section_mut();

    for (id, config) in edit_objs.iter() {
        let obj = {
            let mut obj = None;

            for layer in scly.layers.as_mut_vec().iter_mut() {
                obj = layer.objects.as_mut_vec().iter_mut().find(|_obj| _obj.instance_id & 0x00FFFFFF == id & 0x00FFFFFF);
                if obj.is_some() {
                    break;
                }
            }

            obj.expect(format!("Could not find object 0x{:X} in room 0x{:X}", id, mrea_id).as_str())
        };

        if let Some(value) = config.position {
            set_position(obj, value, false);
        }

        if let Some(value) = config.rotation {
            set_rotation(obj, value, false);
        }

        if let Some(value) = config.scale {
            set_scale(obj, value, false);
        }

        if let Some(value) = config.size {
            set_patterned_size(obj, value, None);
        }

        if let Some(value) = config.speed {
            set_patterned_speed(obj, value, None);
        }

        if let Some(value) = config.damage {
            set_damage(obj, value);   
        }

        if let Some(value) = config.detection_range {
            set_detection_range(obj, value, None);
        }

        if let Some(value) = config.attack_range {
            set_attack_range(obj, value, None);
        }

        if let Some(value) = &config.vulnerability {
            let value = DoorType::from_string(value.clone()).unwrap();
            set_vulnerability(obj, value, None);
        }

        if let Some(values) = &config.vulnerabilities {
            for (index, value) in values {
                let value = DoorType::from_string(value.clone()).unwrap();
                set_vulnerability(obj, value, Some(*index as usize));
            }
        }

        if let Some(value) = config.health {
            set_health(obj, value, None);
        }

        if let Some(values) = &config.healths {
            for (index, value) in values {
                set_health(obj, *value, Some(*index as usize));
            }
        }
    }

    Ok(())
}

/* Interface */

pub fn set_position(obj: &mut structs::SclyObject, value: [f32; 3], relative: bool) {
    if !obj.property_data.supports_position() {
        panic!("object 0x{:X} does not support property \"position\"", obj.instance_id);
    }

    if relative {
        let x = obj.property_data.get_position();
        obj.property_data.set_position(
            [
                x[0] + value[0],
                x[1] + value[1],
                x[2] + value[2],
            ]
        );
    } else {
        obj.property_data.set_position(value);
    }
}

pub fn set_rotation(obj: &mut structs::SclyObject, value: [f32; 3], relative: bool) {
    if !obj.property_data.supports_rotation() {
        panic!("object 0x{:X} does not support property \"rotation\"", obj.instance_id);
    }

    if relative {
        let x = obj.property_data.get_rotation();
        obj.property_data.set_rotation(
            [
                x[0] + value[0],
                x[1] + value[1],
                x[2] + value[2],
            ]
        );
    } else {
        obj.property_data.set_rotation(value);
    }
}

pub fn set_scale(obj: &mut structs::SclyObject, value: [f32; 3], relative: bool) {
    if !obj.property_data.supports_scale() {
        panic!("object 0x{:X} does not support property \"scale\"", obj.instance_id);
    }

    if relative {
        let x = obj.property_data.get_scale();
        obj.property_data.set_scale(
            [
                x[0] * value[0],
                x[1] * value[1],
                x[2] * value[2],
            ]
        );
    } else {
        obj.property_data.set_scale(value);
    }
}

pub fn set_patterned_speed(obj: &mut structs::SclyObject, value: f32, index: Option<usize>) {
    let mut set = false;
    let mut data = get_patterned_infos(obj);
    for i in 0..data.len() {
        if should_skip(i, index) { continue; }
        let x = &mut data[i];
        x.speed *= value;
        x.turn_speed *= value;
        x.average_attack_time *= 1.0/value;
        // x.attack_time_variation *= 1.0/value;
        x.damage_wait_time *= 1.0/value;
        set = true;
    }
    set_patterned_infos(obj, data);

    if !set {
        panic!("object 0x{:X} does not support property \"speed\"", obj.instance_id);
    }
}

pub fn set_patterned_size(obj: &mut structs::SclyObject, value: f32, index: Option<usize>) {
    let mut set = false;
    let mut data = get_patterned_infos(obj);
    for i in 0..data.len() {
        if should_skip(i, index) { continue; }
        let x = &mut data[i];
        x.mass *= value;
        x.half_extent *= value;
        x.height *= value;
        x.step_up_height *= value;
        x.min_attack_range *= value;
        set = true;
    }
    set_patterned_infos(obj, data);

    if !set {
        panic!("object 0x{:X} does not support property \"size\"", obj.instance_id);
    }
}

pub fn set_detection_range(obj: &mut structs::SclyObject, value: f32, index: Option<usize>) {
    let mut set = false;
    let mut data = get_patterned_infos(obj);
    for i in 0..data.len() {
        if should_skip(i, index) { continue; }
        let x = &mut data[i];
        x.detection_range *= value;
        x.detection_height_range *= value;
        x.detection_angle *= value;
        x.player_leash_radius *= value;
        // x.player_leash_time *= value;
        x.leash_radius *= value;
        set = true;
    }
    set_patterned_infos(obj, data);

    if !set {
        panic!("object 0x{:X} does not support property \"detectionRange\"", obj.instance_id);
    }
}

pub fn set_attack_range(obj: &mut structs::SclyObject, value: f32, index: Option<usize>) {
    let mut set = false;
    let mut data = get_patterned_infos(obj);
    for i in 0..data.len() {
        if should_skip(i, index) { continue; }
        let x = &mut data[i];
        x.max_attack_range *= value;
        set = true;
    }
    set_patterned_infos(obj, data);

    if !set {
        panic!("object 0x{:X} does not support property \"attackRange\"", obj.instance_id);
    }
}

pub fn set_vulnerability(obj: &mut structs::SclyObject, value: DoorType, index: Option<usize>) {
    let mut set = false;
    let mut data = get_vulnerabilities(obj);
    for i in 0..data.len() {
        if should_skip(i, index) { continue; }
        data[i] = value.vulnerability();
        set = true;
    }
    set_vulnerabilities(obj, data);

    if !set {
        panic!("object 0x{:X} does not support property \"vulnerability\"", obj.instance_id);
    }
}

pub fn set_health(obj: &mut structs::SclyObject, value: f32, index: Option<usize>) {
    let mut set = false;
    let mut health_infos = get_health_infos(obj);
    for i in 0..health_infos.len() {
        if should_skip(i, index) { continue; }
        health_infos[i].health *= value;
        set = true;
    }
    set_health_infos(obj, health_infos);

    if !set {
        panic!("object 0x{:X} does not support property \"health\"", obj.instance_id);
    }
}

pub fn set_damage(obj: &mut structs::SclyObject, value: f32) {
    let mut set = false;
    let mut infos = get_patterned_infos(obj);
    for i in 0..infos.len() {
        let x = &mut infos[i];
        x.x_damage *= value;
        set = true;
    }
    set_patterned_infos(obj, infos);

    let mut damage_infos = get_damage_infos(obj);
    for i in 0..damage_infos.len() {
        damage_infos[i].damage *= value;
        damage_infos[i].knockback_power *= value;
        set = true;
    }
    set_damage_infos(obj, damage_infos);

    if !set {
        panic!("object 0x{:X} does not support property \"damage\"", obj.instance_id);
    }
}

/* Helpers */

fn should_skip(current: usize, check: Option<usize>) -> bool {
    match check {
        Some(x) => x != current,
        None => false,
    }
}

fn get_patterned_infos(obj: &mut structs::SclyObject) -> Vec<structs::scly_structs::PatternedInfo> {
    if !obj.property_data.supports_patterned_infos() {
        Vec::new()
    } else {
        obj.property_data.get_patterned_infos()
    }
}

fn set_patterned_infos(obj: &mut structs::SclyObject, value: Vec<structs::scly_structs::PatternedInfo>) {
    if value.len() > 0 {
        obj.property_data.set_patterned_infos(value);
    }
}

fn get_damage_infos(obj: &mut structs::SclyObject) -> Vec<structs::scly_structs::DamageInfo> {
    if !obj.property_data.supports_damage_infos() {
        Vec::new()
    } else {
        obj.property_data.get_damage_infos()
    }
}

fn set_damage_infos(obj: &mut structs::SclyObject, value: Vec<structs::scly_structs::DamageInfo>) {
    if value.len() > 0 {
        obj.property_data.set_damage_infos(value);
    }
}

fn get_vulnerabilities(obj: &mut structs::SclyObject) -> Vec<structs::scly_structs::DamageVulnerability> {
    if !obj.property_data.supports_vulnerabilities() {
        Vec::new()
    } else {
        obj.property_data.get_vulnerabilities()
    }
}

fn set_vulnerabilities(obj: &mut structs::SclyObject, value: Vec<structs::scly_structs::DamageVulnerability>) {
    if value.len() > 0 {
        obj.property_data.set_vulnerabilities(value);
    }
}

fn get_health_infos(obj: &mut structs::SclyObject) -> Vec<structs::scly_structs::HealthInfo> {
    if !obj.property_data.supports_health_infos() {
        Vec::new()
    } else {
        obj.property_data.get_health_infos()
    }
}

fn set_health_infos(obj: &mut structs::SclyObject, value: Vec<structs::scly_structs::HealthInfo>) {
    if value.len() > 0 {
        obj.property_data.set_health_infos(value);
    }
}
