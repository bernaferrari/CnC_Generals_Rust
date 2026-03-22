//! Map utility and cache helpers.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use crate::display::image::{get_mapped_image_collection, Image};
use crate::game_text::GameText;
use crate::gui::gadgets::{ListBox, ListBoxItemData};
use crate::gui::shell::Color as WindowColor;
use crate::message_stream::game_message::ICoord2D;

use game_engine::common::ascii_string::AsciiString;
use game_engine::common::crc::Crc;
use game_engine::common::global_data::GLOBAL_DATA;
use game_engine::common::ini::ini::{INILoadType, INI};
use game_engine::common::ini::ini_map_cache::{
    get_map_cache, get_map_cache_mut, init_global_map_cache, set_game_text_provider, Coord3D,
    MapMetaData, Region3D, UnicodeString, WinTimeStamp,
};
use game_engine::common::skirmish_battle_honors::SkirmishBattleHonors;
use game_engine::common::system::file::FileAccess;
use game_engine::common::system::file_system::{get_file_system, FileInfo, FilenameList};
use game_engine::common::system::quoted_printable::ascii_string_to_quoted_printable;

use game_network::gamespy::ladder_defs::{
    set_ladder_map_provider, LadderMapMeta, LadderMapProvider,
};
use gamelogic::system::map_loader::MapLoader;

const MAP_CACHE_NAME: &str = "MapCache.ini";
const MAP_EXTENSION: &str = "map";
const MAP_PREVIEW_DIR_SUFFIX: &str = "MapPreviews/";

#[derive(Default, Debug)]
pub struct TechAndSupplyImages {
    pub tech_positions: Vec<ICoord2D>,
    pub supply_positions: Vec<ICoord2D>,
}

static SUPPLY_AND_TECH_IMAGES: OnceLock<Arc<Mutex<TechAndSupplyImages>>> = OnceLock::new();

pub fn get_supply_and_tech_image_locations() -> Arc<Mutex<TechAndSupplyImages>> {
    SUPPLY_AND_TECH_IMAGES
        .get_or_init(|| Arc::new(Mutex::new(TechAndSupplyImages::default())))
        .clone()
}

#[derive(Debug)]
pub struct MapCache {
    map_dir: String,
    user_map_dir: String,
    seen: HashMap<String, bool>,
    allowed_maps: HashSet<String>,
}

impl MapCache {
    pub fn new() -> Self {
        init_global_map_cache();
        let _ = set_game_text_provider(Arc::new(|tag| Some(GameText::fetch(tag))));
        Self {
            map_dir: "Maps".to_string(),
            user_map_dir: build_user_map_dir(),
            seen: HashMap::new(),
            allowed_maps: HashSet::new(),
        }
    }

    pub fn update_cache(&mut self) {
        {
            let file_system_ref = get_file_system();
            let mut file_system = file_system_ref.lock().unwrap();
            let _ = file_system.create_directory(AsciiString::from(&self.user_map_dir));
        }

        if self.load_user_maps(false) {
            self.write_cache_ini(true);
        }
        self.load_standard_maps();

        let build_map_cache = GLOBAL_DATA
            .read()
            .map(|data| data.writable.build_map_cache)
            .unwrap_or(false);
        if build_map_cache {
            let _ = self.load_user_maps(true);
            self.write_cache_ini(false);
        }
    }

    pub fn get_map_dir(&self) -> &str {
        &self.map_dir
    }

    pub fn get_user_map_dir(&self) -> &str {
        &self.user_map_dir
    }

    pub fn get_map_extension(&self) -> &str {
        MAP_EXTENSION
    }

    pub fn add_shipping_map(&mut self, map_name: &str) {
        self.allowed_maps.insert(map_name.to_lowercase());
    }

    pub fn find_map(&self, map_name: &str) -> Option<MapMetaData> {
        let map_name = map_name.to_lowercase();
        get_map_cache()
            .as_ref()
            .and_then(|cache| cache.get(&map_name))
            .cloned()
    }

    pub fn iter_maps(&self) -> Vec<(String, MapMetaData)> {
        get_map_cache()
            .as_ref()
            .map(|cache| {
                cache
                    .iter()
                    .map(|(name, meta)| (name.clone(), meta.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn load_standard_maps(&self) {
        let map_cache_path = PathBuf::from(&self.map_dir).join(MAP_CACHE_NAME);
        let map_cache_path = map_cache_path.to_string_lossy().into_owned();
        let map_cache_exists = {
            let file_system_ref = get_file_system();
            let file_system = file_system_ref.lock().unwrap();
            file_system.does_file_exist(&map_cache_path)
        };
        if !map_cache_exists {
            return;
        }
        let mut ini = INI::new();
        let _ = ini.load(&map_cache_path, INILoadType::Overwrite);
    }

    fn load_user_maps(&mut self, build_map_cache: bool) -> bool {
        let map_dir = if build_map_cache {
            self.map_dir.clone()
        } else {
            self.user_map_dir.clone()
        };

        if !build_map_cache {
            let map_cache_path = PathBuf::from(&map_dir)
                .join(MAP_CACHE_NAME)
                .to_string_lossy()
                .into_owned();
            let map_cache_exists = {
                let file_system_ref = get_file_system();
                let file_system = file_system_ref.lock().unwrap();
                file_system.does_file_exist(&map_cache_path)
            };
            if map_cache_exists {
                let mut ini = INI::new();
                let _ = ini.load(&map_cache_path, INILoadType::Overwrite);
            }
        }

        self.mark_all_unseen();

        let file_list = collect_map_files(&map_dir);
        let mut parsed_map = false;
        for filename in file_list {
            let map_path = normalize_path(&filename);
            if !is_map_in_expected_folder(&map_path) {
                continue;
            }

            let map_base = map_base_name(&map_path);
            if build_map_cache && !self.allowed_maps.is_empty() {
                if !self.allowed_maps.contains(&map_base) {
                    continue;
                }
            }

            let file_info = {
                let file_system_ref = get_file_system();
                let file_system = file_system_ref.lock().unwrap();
                file_system.get_file_info(&AsciiString::from(&map_path))
            };
            if let Some(info) = file_info {
                self.seen.insert(map_path.clone(), true);
                parsed_map |= self.add_map(&map_dir, &map_path, &info, build_map_cache);
            }
        }

        if self.clear_unseen_maps(&map_dir) {
            return true;
        }

        parsed_map
    }

    fn mark_all_unseen(&mut self) {
        self.seen.clear();
        if let Some(cache) = get_map_cache() {
            for name in cache.get_map_names() {
                self.seen.insert(name.clone(), false);
            }
        }
    }

    fn clear_unseen_maps(&mut self, map_dir: &str) -> bool {
        let mut removed = false;
        let map_dir_norm = normalize_path(map_dir);
        if let Some(mut cache) = get_map_cache_mut() {
            let names: Vec<String> = cache.get_map_names().into_iter().cloned().collect();
            for name in names {
                let lower = normalize_path(&name);
                if !self.seen.get(&lower).copied().unwrap_or(false)
                    && lower.starts_with(&map_dir_norm)
                {
                    cache.remove(&lower);
                    removed = true;
                }
            }
        }
        removed
    }

    fn add_map(
        &self,
        _dir_name: &str,
        map_path: &str,
        file_info: &FileInfo,
        is_official: bool,
    ) -> bool {
        let lower_path = normalize_path(map_path);

        if let Some(cache) = get_map_cache() {
            if let Some(existing) = cache.get(&lower_path) {
                if existing.filesize == file_info.size_low as u32 && existing.crc != 0 {
                    return false;
                }
            }
        }

        let file_bytes = match read_file_bytes(map_path) {
            Ok(bytes) => bytes,
            Err(_) => return false,
        };
        let mut loader = MapLoader::new();
        if loader.load_map_from_bytes(&file_bytes).is_err() {
            return false;
        }

        let mut metadata = MapMetaData::new();
        metadata.file_name = lower_path.clone();
        metadata.filesize = file_info.size_low as u32;
        metadata.is_official = is_official;
        metadata.timestamp = WinTimeStamp::new(file_info.timestamp_low, file_info.timestamp_high);

        let num_players = loader.count_start_spots() as i32;
        metadata.num_players = num_players;
        metadata.is_multiplayer = num_players >= 2;
        metadata.extent = map_extent(loader.get_heightmap());
        metadata.waypoints = loader
            .get_waypoints()
            .iter()
            .map(|(k, v)| (k.clone(), to_ini_coord3d(*v)))
            .collect();
        metadata.supply_positions = loader
            .get_supply_positions()
            .iter()
            .map(|v| to_ini_coord3d(*v))
            .collect();
        metadata.tech_positions = loader
            .get_tech_positions()
            .iter()
            .map(|v| to_ini_coord3d(*v))
            .collect();
        metadata.crc = Crc::from_buffer(&file_bytes).get();

        metadata.name_lookup_tag = loader
            .get_world_dict()
            .get("mapName")
            .cloned()
            .unwrap_or_default();

        let display_name = compute_display_name(&metadata, map_path);
        metadata.display_name = display_name;

        if let Some(mut cache) = get_map_cache_mut() {
            cache.insert(lower_path, metadata);
        }

        true
    }

    fn write_cache_ini(&self, user_dir: bool) {
        let map_dir = if !user_dir {
            self.map_dir.clone()
        } else {
            self.user_map_dir.clone()
        };

        let file_system_ref = get_file_system();
        let mut file_system = file_system_ref.lock().unwrap();
        let _ = file_system.create_directory(AsciiString::from(&map_dir));

        let map_cache_path = PathBuf::from(&map_dir).join(MAP_CACHE_NAME);
        let mut file = match file_system.open_file(
            map_cache_path.to_string_lossy().as_ref(),
            FileAccess::WRITE
                .combine(FileAccess::CREATE)
                .combine(FileAccess::TRUNCATE)
                .combine(FileAccess::TEXT),
        ) {
            Some(file) => file,
            None => return,
        };

        let _ = file.print(&format!(
            "; FILE: {} /////////////////////////////////////////////////////////////\n",
            map_cache_path.to_string_lossy()
        ));
        let _ = file.print("; This INI file is auto-generated - do not modify\n");
        let _ = file.print(
            "; /////////////////////////////////////////////////////////////////////////////\n",
        );

        let map_dir_norm = normalize_path(&map_dir);
        if let Some(cache) = get_map_cache() {
            for (name, meta) in cache.iter() {
                let lower = normalize_path(name);
                if !lower.starts_with(&map_dir_norm) {
                    continue;
                }
                let qp_name = ascii_string_to_quoted_printable(name);
                let _ = file.print(&format!("\nMapCache {}\n", qp_name));
                let _ = file.print(&format!("  fileSize = {}\n", meta.filesize));
                let _ = file.print(&format!("  fileCRC = {}\n", meta.crc));
                let _ = file.print(&format!(
                    "  timestampLo = {}\n",
                    meta.timestamp.low_time_stamp
                ));
                let _ = file.print(&format!(
                    "  timestampHi = {}\n",
                    meta.timestamp.high_time_stamp
                ));
                let _ = file.print(&format!(
                    "  isOfficial = {}\n",
                    if meta.is_official { "yes" } else { "no" }
                ));
                let _ = file.print(&format!(
                    "  isMultiplayer = {}\n",
                    if meta.is_multiplayer { "yes" } else { "no" }
                ));
                let _ = file.print(&format!("  numPlayers = {}\n", meta.num_players));
                let _ = file.print(&format!(
                    "  extentMin = X:{:.2} Y:{:.2} Z:{:.2}\n",
                    meta.extent.lo.x, meta.extent.lo.y, meta.extent.lo.z
                ));
                let _ = file.print(&format!(
                    "  extentMax = X:{:.2} Y:{:.2} Z:{:.2}\n",
                    meta.extent.hi.x, meta.extent.hi.y, meta.extent.hi.z
                ));
                let _ = file.print(&format!("  nameLookupTag = {}\n", meta.name_lookup_tag));

                for (waypoint, pos) in &meta.waypoints {
                    let _ = file.print(&format!(
                        "  {} = X:{:.2} Y:{:.2} Z:{:.2}\n",
                        waypoint, pos.x, pos.y, pos.z
                    ));
                }
                for pos in &meta.tech_positions {
                    let _ = file.print(&format!(
                        "  techPosition = X:{:.2} Y:{:.2} Z:{:.2}\n",
                        pos.x, pos.y, pos.z
                    ));
                }
                for pos in &meta.supply_positions {
                    let _ = file.print(&format!(
                        "  supplyPosition = X:{:.2} Y:{:.2} Z:{:.2}\n",
                        pos.x, pos.y, pos.z
                    ));
                }
                let _ = file.print("END\n\n");
            }
        }
    }

    pub fn has_map(&self, map_name: &str) -> bool {
        let map_name = map_name.to_lowercase();
        get_map_cache()
            .as_ref()
            .is_some_and(|cache| cache.get(&map_name).is_some())
    }

    pub fn has_map_cpp_key(&self, map_name: &str) -> bool {
        self.has_map(map_name)
    }
}

static THE_MAP_CACHE: OnceLock<Arc<Mutex<MapCache>>> = OnceLock::new();
static LADDER_PROVIDER_SET: OnceLock<()> = OnceLock::new();

pub fn get_map_cache_manager() -> Arc<Mutex<MapCache>> {
    let cache = THE_MAP_CACHE
        .get_or_init(|| Arc::new(Mutex::new(MapCache::new())))
        .clone();
    register_ladder_map_provider();
    cache
}

struct MapCacheLadderProvider;

impl LadderMapProvider for MapCacheLadderProvider {
    fn map_dir(&self) -> String {
        let cache = get_map_cache_manager();
        let cache_guard = cache.lock().unwrap();
        cache_guard.map_dir.clone()
    }

    fn find_map(&self, map_path: &str) -> Option<LadderMapMeta> {
        let cache = get_map_cache_manager();
        let mut cache_guard = cache.lock().unwrap();
        cache_guard.update_cache();
        cache_guard.find_map(map_path).map(|meta| LadderMapMeta {
            display_name: meta.display_name.as_str().to_string(),
            num_players: meta.num_players as i32,
        })
    }
}

fn register_ladder_map_provider() {
    if LADDER_PROVIDER_SET.get().is_some() {
        return;
    }
    let provider: Arc<dyn LadderMapProvider> = Arc::new(MapCacheLadderProvider);
    set_ladder_map_provider(provider);
    let _ = LADDER_PROVIDER_SET.set(());
}

pub fn populate_map_listbox(
    listbox: &mut ListBox,
    use_system_maps: bool,
    is_multiplayer: bool,
    map_to_select: Option<&str>,
) -> i32 {
    listbox.clear();
    populate_map_listbox_no_reset(listbox, use_system_maps, is_multiplayer, map_to_select)
}

pub fn populate_map_listbox_no_reset(
    listbox: &mut ListBox,
    use_system_maps: bool,
    is_multiplayer: bool,
    map_to_select: Option<&str>,
) -> i32 {
    const EASY_AI: i32 = 2;
    const MED_AI: i32 = 3;
    const BRUTAL_AI: i32 = 4;

    let cache = get_map_cache_manager();
    let mut cache_guard = cache.lock().unwrap();
    cache_guard.update_cache();

    let map_dir = if use_system_maps {
        cache_guard.map_dir.clone()
    } else {
        cache_guard.user_map_dir.clone()
    };
    let map_dir_norm = normalize_path(&map_dir);

    let mut entries = Vec::new();
    for (name, meta) in cache_guard.iter_maps() {
        let name_norm = normalize_path(&name);
        if !name_norm.starts_with(&map_dir_norm) {
            continue;
        }
        if meta.is_multiplayer != is_multiplayer || meta.display_name.is_empty() {
            continue;
        }
        if is_banned_map(&name_norm) {
            continue;
        }
        entries.push((
            meta.num_players,
            meta.display_name.as_str().to_string(),
            name,
        ));
    }

    entries.sort_by(|a, b| {
        let players = a.0.cmp(&b.0);
        if players != std::cmp::Ordering::Equal {
            return players;
        }
        a.1.to_lowercase().cmp(&b.1.to_lowercase())
    });

    let num_columns = listbox.columns().max(1) as usize;
    let mut honors = if num_columns > 1 && is_multiplayer {
        Some(SkirmishBattleHonors::new())
    } else {
        None
    };
    let mut easy_image: Option<String> = None;
    let mut medium_image: Option<String> = None;
    let mut brutal_image: Option<String> = None;
    let mut max_brutal_image: Option<String> = None;
    let mut image_width: u32 = 10;
    let mut image_height: u32 = 10;
    if honors.is_some() {
        if let Some(collection) = get_mapped_image_collection().try_read() {
            if let Some(image) = collection.find_image_by_name("Star-Bronze") {
                easy_image = Some(image.get_name().to_string());
            }
            if let Some(image) = collection.find_image_by_name("Star-Silver") {
                medium_image = Some(image.get_name().to_string());
            }
            if let Some(image) = collection.find_image_by_name("Star-Gold") {
                brutal_image = Some(image.get_name().to_string());
                image_width = image.get_image_width().max(1) as u32;
                image_height = image_width;
            }
            if let Some(image) = collection.find_image_by_name("RedYell_Star") {
                max_brutal_image = Some(image.get_name().to_string());
            }
        }
        let column_widths = listbox.column_widths();
        if let Some(first_width) = column_widths.get(0) {
            image_width = image_width.min(*first_width);
            image_height = image_width;
        }
    }

    let mut selection_index: i32 = 0;
    let map_to_select = map_to_select.map(normalize_path);
    for (index, entry) in entries.iter().enumerate() {
        let id = index as i32;
        let row = listbox.add_item_with_data_and_color(
            id,
            &entry.1,
            Some(ListBoxItemData::Text(entry.2.clone())),
            Some(WindowColor::new(255, 255, 255, 255)),
        );
        if num_columns > 1 {
            let (image_name, item_data) = if let Some(honors) = honors.as_ref() {
                let num_easy = honors.get_endurance_medal(&entry.2, EASY_AI);
                let num_med = honors.get_endurance_medal(&entry.2, MED_AI);
                let num_brutal = honors.get_endurance_medal(&entry.2, BRUTAL_AI);
                let max_brutal_slots = entries[index].0.saturating_sub(1) as i32;
                if num_brutal > 0 {
                    if num_brutal == max_brutal_slots {
                        (max_brutal_image.clone(), 4)
                    } else {
                        (brutal_image.clone(), 3)
                    }
                } else if num_med > 0 {
                    (medium_image.clone(), 2)
                } else if num_easy > 0 {
                    (easy_image.clone(), 1)
                } else {
                    (None, 0)
                }
            } else {
                (None, 0)
            };

            let image_name = image_name.unwrap_or_default();
            let _ = listbox.set_item_column_data(
                row,
                0,
                ListBoxItemData::Image {
                    name: image_name,
                    width: image_width,
                    height: image_height,
                    text: None,
                },
            );
            let text_column = num_columns.saturating_sub(1);
            if text_column != 0 {
                let _ = listbox.set_item_column_data(
                    row,
                    text_column,
                    ListBoxItemData::Text(entry.1.clone()),
                );
                let _ = listbox.set_item_column_color(
                    row,
                    text_column,
                    Some(WindowColor::new(255, 255, 255, 255)),
                );
            }
            let _ = listbox.set_item_column_user_data(
                row,
                1,
                Some(ListBoxItemData::Integer(item_data)),
            );
        }
        if map_to_select
            .as_ref()
            .map(|s| s == &normalize_path(&entry.2))
            == Some(true)
        {
            selection_index = index as i32;
        }
    }

    if !entries.is_empty() {
        listbox.set_selected_indices(&[selection_index as usize]);
        let top = listbox.get_top_visible_entry();
        let bottom = listbox.get_bottom_visible_entry();
        if selection_index as usize >= bottom {
            let rows = bottom.saturating_sub(top.max(0) as usize).max(1);
            let new_top = selection_index.saturating_sub(rows as i32 / 2).max(0);
            listbox.set_top_visible_entry(new_top);
        }
    }

    selection_index
}

pub fn is_valid_map(map_name: &str, is_multiplayer: bool) -> bool {
    let cache = get_map_cache_manager();
    let mut cache_guard = cache.lock().unwrap();
    cache_guard.update_cache();
    let map_name = normalize_path(map_name);
    cache_guard
        .find_map(&map_name)
        .map(|meta| meta.is_multiplayer == is_multiplayer)
        .unwrap_or(false)
}

pub fn get_default_map(is_multiplayer: bool) -> String {
    let cache = get_map_cache_manager();
    let mut cache_guard = cache.lock().unwrap();
    cache_guard.update_cache();
    let mut names: Vec<_> = cache_guard
        .iter_maps()
        .into_iter()
        .filter(|(_, meta)| meta.is_multiplayer == is_multiplayer)
        .map(|(name, _)| name)
        .collect();
    names.sort();
    names.first().cloned().unwrap_or_default()
}

pub fn get_default_official_map() -> String {
    let cache = get_map_cache_manager();
    let mut cache_guard = cache.lock().unwrap();
    cache_guard.update_cache();
    let mut names: Vec<_> = cache_guard
        .iter_maps()
        .into_iter()
        .filter(|(_, meta)| meta.is_multiplayer && meta.is_official)
        .map(|(name, _)| name)
        .collect();
    names.sort();
    names.first().cloned().unwrap_or_default()
}

pub fn is_official_map(map_name: &str) -> bool {
    let cache = get_map_cache_manager();
    let mut cache_guard = cache.lock().unwrap();
    cache_guard.update_cache();
    let map_name = normalize_path(map_name);
    cache_guard
        .find_map(&map_name)
        .map(|meta| meta.is_official)
        .unwrap_or(false)
}

pub fn get_map_preview_image(map_name: &str) -> Option<String> {
    let map_name = map_name.trim();
    if map_name.is_empty() {
        return None;
    }

    let tga_path = replace_extension(map_name, "tga");
    if !file_exists(&tga_path) {
        return None;
    }

    let portable_name = sanitize_preview_name(map_name);
    let preview_filename = format!("{}.tga", portable_name);
    let preview_dir = build_map_preview_dir();
    let preview_path = PathBuf::from(&preview_dir).join(&preview_filename);

    let preview_file_path = preview_path.to_string_lossy();
    if !file_exists(&preview_file_path) {
        let _ = copy_from_file_system(&tga_path, preview_file_path.as_ref());
    }

    let collection = get_mapped_image_collection();
    let mut collection_guard = collection.write();
    if collection_guard
        .find_image_by_name(&portable_name)
        .is_none()
    {
        if let Ok(image) = Image::load_from_file(&preview_path, Some(portable_name.clone())) {
            collection_guard.add_image(image);
        }
    }

    Some(portable_name)
}

pub fn parse_map_preview_chunk(
    _data: &[u8],
    _extent: Region3D,
    _target: &mut TechAndSupplyImages,
) -> bool {
    false
}

pub fn find_draw_positions(
    start_x: i32,
    start_y: i32,
    width: i32,
    height: i32,
    extent: Region3D,
) -> (ICoord2D, ICoord2D) {
    let extent_width = extent.hi.x - extent.lo.x;
    let extent_height = extent.hi.y - extent.lo.y;
    let ratio_width = extent_width / width as f32;
    let ratio_height = extent_height / height as f32;

    let mut ul = ICoord2D { x: 0, y: 0 };
    let mut lr = ICoord2D { x: 0, y: 0 };

    if ratio_width >= ratio_height {
        let radar_x = extent_width / ratio_width;
        let radar_y = extent_height / ratio_width;
        ul.x = 0;
        ul.y = ((height as f32 - radar_y) / 2.0_f32) as i32;
        lr.x = radar_x as i32;
        lr.y = height - ul.y;
    } else {
        let radar_x = extent_width / ratio_height;
        let radar_y = extent_height / ratio_height;
        ul.x = ((width as f32 - radar_x) / 2.0_f32) as i32;
        ul.y = 0;
        lr.x = width - ul.x;
        lr.y = radar_y as i32;
    }

    ul.x += start_x;
    ul.y += start_y;
    lr.x += start_x;
    lr.y += start_y;

    (ul, lr)
}

pub fn would_map_transfer(map_name: &str) -> bool {
    let map_name = normalize_path(map_name);
    let cache = get_map_cache_manager();
    let cache_guard = cache.lock().unwrap();
    map_name.starts_with(&normalize_path(&cache_guard.user_map_dir))
}

fn build_user_map_dir() -> String {
    let user_dir = GLOBAL_DATA
        .read()
        .map(|data| data.get_user_data_dir().to_string())
        .unwrap_or_default();
    let mut base = if user_dir.is_empty() {
        "UserData/".to_string()
    } else {
        user_dir
    };
    if !base.ends_with('/') && !base.ends_with('\\') {
        base.push('/');
    }
    base.push_str("Maps");
    base
}

fn build_map_preview_dir() -> String {
    let user_dir = GLOBAL_DATA
        .read()
        .map(|data| data.get_user_data_dir().to_string())
        .unwrap_or_default();
    let mut base = if user_dir.is_empty() {
        "UserData/".to_string()
    } else {
        user_dir
    };
    if !base.ends_with('/') && !base.ends_with('\\') {
        base.push('/');
    }
    base.push_str(MAP_PREVIEW_DIR_SUFFIX);
    base
}

fn normalize_path(path: &str) -> String {
    path.replace('\\', "/").to_lowercase()
}

fn replace_extension(path: &str, new_ext: &str) -> String {
    let mut path = path.to_string();
    if let Some(pos) = path.rfind('.') {
        path.truncate(pos + 1);
        path.push_str(new_ext);
        return path;
    }
    format!("{}.{}", path, new_ext)
}

fn is_map_in_expected_folder(path: &str) -> bool {
    let path = normalize_path(path);
    let filename = match path.rsplit('/').next() {
        Some(name) => name,
        None => return false,
    };
    let stem = filename.trim_end_matches(&format!(".{}", MAP_EXTENSION));
    let expected = format!("{}/{}", stem, filename);
    path.ends_with(&expected)
}

fn map_base_name(path: &str) -> String {
    let filename = path
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .trim_end_matches(&format!(".{}", MAP_EXTENSION));
    filename.to_lowercase()
}

fn compute_display_name(metadata: &MapMetaData, map_path: &str) -> UnicodeString {
    let map_name_tag = metadata.name_lookup_tag.clone();

    if map_name_tag.is_empty() {
        let base = path_basename(map_path);
        let mut display = UnicodeString::new();
        display.translate(&base);
        if metadata.num_players >= 2 {
            let mut ext = UnicodeString::new();
            ext.format(" (%d)", metadata.num_players);
            display.concat(&ext);
        }
        return display;
    }

    let map_str_path = map_string_file_for_map(map_path);
    if let Some(path) = map_str_path {
        let _ = GameText::init_map_string_file(&path);
    }
    let localized = GameText::fetch(&map_name_tag);
    GameText::reset();

    let mut display = UnicodeString::new();
    display.translate(&localized);
    if metadata.num_players >= 2 {
        let mut ext = UnicodeString::new();
        ext.format(" (%d)", metadata.num_players);
        display.concat(&ext);
    }
    display
}

fn path_basename(path: &str) -> String {
    path.rsplit(|c| c == '/' || c == '\\')
        .next()
        .unwrap_or(path)
        .to_string()
}

fn map_string_file_for_map(map_path: &str) -> Option<String> {
    let mut dir = map_path.to_string();
    if let Some(pos) = dir.rfind('.') {
        dir.truncate(pos);
    }
    dir.push_str("/map.str");
    if file_exists(&dir) {
        return Some(dir);
    }
    let mut alt = dir.clone();
    if let Some(pos) = alt.rfind('/') {
        alt.replace_range(pos + 1.., "Map.str");
    }
    if file_exists(&alt) {
        return Some(alt);
    }
    None
}

fn map_extent(heightmap: &gamelogic::system::map_loader::HeightMap) -> Region3D {
    let (dx, dy) = heightmap.get_playable_dimensions();
    let max_x = dx as f32 * gamelogic::system::map_loader::MAP_XY_FACTOR;
    let max_y = dy as f32 * gamelogic::system::map_loader::MAP_XY_FACTOR;
    Region3D {
        lo: Coord3D::new(0.0, 0.0, 0.0),
        hi: Coord3D::new(max_x, max_y, 0.0),
    }
}

pub fn is_map_cached(map_name: &str) -> bool {
    let cache = get_map_cache_manager();
    let mut cache_guard = cache.lock().unwrap();
    cache_guard.update_cache();
    cache_guard.has_map(map_name)
}

pub fn refresh_map_cache() {
    let cache = get_map_cache_manager();
    let mut cache_guard = cache.lock().unwrap();
    cache_guard.update_cache();
}

pub fn is_map_cached_without_refresh(map_name: &str) -> bool {
    if map_name.is_empty() {
        return false;
    }

    let cache = get_map_cache_manager();
    let cache_guard = cache.lock().unwrap();
    cache_guard.has_map_cpp_key(map_name)
}

fn to_ini_coord3d(coord: gamelogic::system::map_loader::Coord3D) -> Coord3D {
    Coord3D::new(coord.x, coord.y, coord.z)
}

fn read_file_bytes(filename: &str) -> Result<Vec<u8>, std::io::Error> {
    let file_system_ref = get_file_system();
    let mut file_system = file_system_ref.lock().unwrap();
    let mut file = file_system
        .open_file(filename, FileAccess::READ.combine(FileAccess::BINARY))
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "file not found"))?;
    file.read_entire_and_close()
}

fn copy_from_file_system(source: &str, dest: &str) -> Result<(), std::io::Error> {
    let data = read_file_bytes(source)?;
    let file_system_ref = get_file_system();
    let mut file_system = file_system_ref.lock().unwrap();
    if let Some(mut file) = file_system.open_file(
        dest,
        FileAccess::WRITE
            .combine(FileAccess::CREATE)
            .combine(FileAccess::TRUNCATE)
            .combine(FileAccess::BINARY),
    ) {
        let _ = file.write(&data)?;
        file.close();
        return Ok(());
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::NotFound,
        "destination file system backend unavailable",
    ))
}

fn file_exists(path: &str) -> bool {
    let file_system_ref = get_file_system();
    let file_system = file_system_ref.lock().unwrap();
    file_system.does_file_exist(path)
}

fn sanitize_preview_name(map_name: &str) -> String {
    let mut name = map_name.to_string();
    if let Some(pos) = name.rfind('.') {
        name.truncate(pos);
    }
    name.chars()
        .map(|c| {
            if c == '\\' || c == '/' || c == ':' {
                '_'
            } else {
                c
            }
        })
        .collect::<String>()
}

fn is_banned_map(map_name: &str) -> bool {
    let map_name = normalize_path(map_name);
    map_name.ends_with("maps/armored fury/armored fury.map")
        || map_name.ends_with("maps/scorched earth/scorched earth.map")
}

fn collect_map_files(map_dir: &str) -> Vec<String> {
    let mut filename_list = FilenameList::new();
    {
        let file_system_ref = get_file_system();
        let file_system = file_system_ref.lock().unwrap();
        file_system.get_file_list_in_directory(
            &AsciiString::from(map_dir),
            &AsciiString::from(&format!("*.{}", MAP_EXTENSION)),
            &mut filename_list,
            true,
        );
    }
    filename_list
        .into_iter()
        .map(|f| f.to_string())
        .collect::<Vec<_>>()
}
