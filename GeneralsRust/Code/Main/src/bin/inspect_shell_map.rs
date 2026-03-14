use anyhow::Result;
use gamelogic::scripting::core::{
    Condition, OrCondition, Script, ScriptAction, ScriptGroup, ScriptList,
};
use generals_main::game_logic::script_loader::{
    find_map_file, load_chunky_map, load_map_scripts, parse_map_settings,
};
use generals_main::game_logic::{GameLogic, GameMode};

fn print_condition_chain(condition: Option<&Condition>, depth: usize) {
    let mut current = condition;
    while let Some(cond) = current {
        println!(
            "{}condition type={:?} parms={:?}",
            " ".repeat(depth),
            cond.condition_type,
            &cond.parameters[..cond.num_parms]
        );
        current = cond.get_next();
    }
}

fn print_or_conditions(or_condition: Option<&OrCondition>, depth: usize) {
    let mut current = or_condition;
    while let Some(or_cond) = current {
        print_condition_chain(or_cond.get_first_and_condition(), depth);
        current = or_cond.get_next_or_condition();
    }
}

fn print_actions(action: Option<&ScriptAction>, depth: usize) {
    let mut current = action;
    while let Some(act) = current {
        println!(
            "{}action type={:?} parms={:?}",
            " ".repeat(depth),
            act.action_type,
            &act.parameters[..act.num_parms]
        );
        current = act.get_next();
    }
}

fn dump_matching_script(script: &Script, group_name: Option<&str>, group_active: Option<bool>) {
    let interesting = script.script_name == "Move Camera"
        || script.script_name == "Restart Camera Script"
        || script.script_name == "Restart Camera"
        || script.script_name == "Restart Camera Really"
        || script.script_name == "Unshroud"
        || script.script_name == "Turn off Sirens"
        || script.script_name.contains("Camera");
    if interesting {
        println!(
            "script name={} active={} one_shot={} delay={} group={:?} group_active={:?}",
            script.script_name,
            script.is_active(),
            script.is_one_shot(),
            script.delay_evaluation_seconds,
            group_name,
            group_active
        );
    }
    if matches!(
        script.script_name.as_str(),
        "Move Camera" | "Restart Camera Script" | "Restart Camera" | "Restart Camera Really"
    ) {
        println!("script_detail_name={}", script.script_name);
        println!(
            "script_detail_condition_comment={}",
            script.condition_comment
        );
        println!("script_detail_action_comment={}", script.action_comment);
        print_or_conditions(script.get_or_condition(), 2);
        print_actions(script.get_action(), 2);
    }
    if let Some(next) = script.get_next() {
        dump_matching_script(next, group_name, group_active);
    }
}

fn dump_matching_group(group: &ScriptGroup) {
    if let Some(script) = group.get_script() {
        dump_matching_script(script, Some(group.get_name()), Some(group.is_active()));
    }
    if let Some(next) = group.get_next() {
        dump_matching_group(next);
    }
}

fn dump_matching_list(list: &ScriptList) {
    if let Some(script) = list.get_script() {
        dump_matching_script(script, None, None);
    }
    if let Some(group) = list.get_script_group() {
        dump_matching_group(group);
    }
}

fn main() -> Result<()> {
    let map_name = "Maps\\ShellMapMD\\ShellMapMD.map";
    let mut logic = GameLogic::initialize();
    logic.start_new_game(GameMode::Skirmish);

    println!("map_name={map_name}");
    println!("resolved={:?}", find_map_file(map_name));

    if let Some(chunky) = load_chunky_map(map_name)? {
        let mut labels: Vec<_> = chunky.toc.values().cloned().collect();
        labels.sort();
        labels.dedup();
        println!("toc_labels_count={}", labels.len());
        for label in labels.iter().take(200) {
            println!("label={label}");
        }

        let body = &chunky.bytes[chunky.body_offset..];
        let mut pos = 0usize;
        while pos + 10 <= body.len() {
            let label_id =
                u32::from_le_bytes([body[pos], body[pos + 1], body[pos + 2], body[pos + 3]]);
            let version = u16::from_le_bytes([body[pos + 4], body[pos + 5]]);
            let size =
                i32::from_le_bytes([body[pos + 6], body[pos + 7], body[pos + 8], body[pos + 9]]);
            pos += 10;
            if size < 0 || pos + size as usize > body.len() {
                break;
            }
            let payload = &body[pos..pos + size as usize];
            pos += size as usize;
            if chunky.toc.get(&label_id).map(String::as_str) == Some("GlobalLighting") {
                println!("global_lighting_version={version}");
                println!("global_lighting_size={}", payload.len());
                let mut first_words = Vec::new();
                for chunk in payload.chunks_exact(4).take(24) {
                    first_words.push(u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]));
                }
                println!("global_lighting_words={first_words:?}");
                break;
            }
        }
    }

    let meta = parse_map_settings(map_name)?;
    println!("objects={}", meta.objects.len());
    println!("heightmap_hint={:?}", meta.heightmap_path);
    println!("world_min={:?}", meta.world_min);
    println!("world_max={:?}", meta.world_max);
    println!("initial_camera_position={:?}", meta.initial_camera_position);
    println!("skybox={:?}", meta.skybox_textures);
    println!("ambient={:?}", meta.ambient_color);
    println!("sun={:?}", meta.sun_color);
    println!("sky={:?}", meta.sky_color);
    println!("sun_dir={:?}", meta.sun_direction);

    if let Some(loaded_scripts) = load_map_scripts(map_name)? {
        println!("script_lists={}", loaded_scripts.script_lists.len());
        println!("total_scripts={}", loaded_scripts.total_scripts);
        for list in &loaded_scripts.script_lists {
            dump_matching_list(list);
        }
    }

    let loaded = logic.load_map(map_name);
    println!("load_map_returned={loaded}");
    println!("logic_objects={}", logic.get_objects().len());
    for object in logic.get_objects().values().take(20) {
        println!(
            "object id={} template={} model={} pos={:?}",
            object.id,
            object.template_name,
            object.get_template().get_model_name(),
            object.get_position()
        );
    }

    Ok(())
}
