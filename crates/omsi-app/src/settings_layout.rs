//! How the Options window is laid out for its users (the custom fork's): pages by what a
//! player wants to do, sections inside them, and names and descriptions that say plainly
//! what each setting does. `game_lists::options_pages` makes the rows as ever; `regroup`
//! sorts them into this layout by their actions, so a setting the game gains later is not
//! lost: it is shown under "More" on the page its old one turned into.

/// The rows of a settings page: (label, action), as `game_lists` makes them.
pub type Rows = Vec<(String, String)>;

/// One setting in the layout: its action, and its name and description here ("" keeps the
/// row's own).
#[derive(Debug, Clone, Copy)]
pub struct Item {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
}

const fn it(id: &'static str, name: &'static str, desc: &'static str) -> Item {
    Item { id, name, desc }
}

/// The AI drivers' setting `name` as it is (its name and description from `ai_drivers`).
const fn ai(id: &'static str) -> Item {
    Item { id, name: "", desc: "" }
}

pub struct Section {
    /// The heading over its rows ("" none).
    pub title: &'static str,
    pub items: &'static [Item],
}

pub struct PageDef {
    pub title: &'static str,
    pub sections: &'static [Section],
    /// The old pages whose rows not placed above come here (under "More").
    pub takes_from: &'static [&'static str],
}

const fn sec(title: &'static str, items: &'static [Item]) -> Section {
    Section { title, items }
}

/// What the game says of a setting that needs a restart (`game_lists::options_pages`).
const LATER: &str = "Takes effect when the game starts the next time";
const AFTER_RESTART: &str = "Applies after a restart";

/// The Options window.
pub const OPTIONS: &[PageDef] = &[
    PageDef {
        title: "Driving",
        takes_from: &["Gameplay"],
        sections: &[
            sec("Collisions", &[
                it("coll_vehicles", "Crash into other vehicles", "Your bus can hit cars, lorries and other buses"),
                it("coll_objects", "Crash into buildings and objects", "Walls, lamp posts, fences and the like stop your bus"),
                it("collision_pedestrians", "Hit pedestrians", "People in the way can be knocked down"),
            ]),
            sec("Your vehicle", &[
                it("sel maintenance", "Vehicle wear", "How quickly parts wear out and break down"),
                it("get_up", "Walk around (Ctrl+Shift+G)", "Get out of the driver's seat and walk about the bus and the map"),
            ]),
            sec("Passengers", &[
                it("sel boarding", "How passengers pay", "Whether you sell the tickets or they pay and board by themselves"),
                it("exact_fare", "Passengers pay the exact fare", "Nobody needs change at the cash desk"),
                it("sel pax_voices", "Passenger voices", "What the passengers say to you"),
            ]),
        ],
    },
    PageDef {
        title: "Traffic",
        takes_from: &["AI lights", "AI hazards & horn", "AI driver habits"],
        sections: &[
            sec("Amount", &[
                it("sel ai_unsched_factor", "Random traffic", "How many cars and lorries drive about"),
                it("sel ai_max_scheduled", "Timetable buses", "How many other buses run on the map's timetable"),
                it("sel ai_max_parked", "Parked cars", "How many cars stand at the kerbs and in car parks"),
            ]),
            sec("AI drivers", &[
                it("aid:enabled", "Realistic AI drivers", "Cars switch their lights on by the weather, use hazard lights, honk and make small mistakes"),
                it("aid:style", "Driving style", "How firmly cars and lorries take bends and junctions: calm, normal or brisk"),
            ]),
            sec("Headlights", &[ai("aid:always_on"), ai("aid:bright_min"), ai("aid:bright_max"), ai("aid:bright_hyst")]),
            sec("Headlights in rain, snow and fog", &[ai("aid:precip_min"), ai("aid:precip_max"), ai("aid:snow_factor"), ai("aid:fog_min_m"), ai("aid:fog_max_m")]),
            sec("How quickly drivers react", &[ai("aid:on_delay_max"), ai("aid:off_delay_min"), ai("aid:off_delay_max")]),
            sec("Rear fog lamp", &[ai("aid:rear_fog"), ai("aid:rear_fog_m")]),
            sec("Hazard lights", &[ai("aid:hazard"), ai("aid:hazard_decel"), ai("aid:hazard_speed"), ai("aid:hazard_hold_min"), ai("aid:hazard_hold_max")]),
            sec("Screeching tyres", &[ai("aid:screech"), ai("aid:screech_decel")]),
            sec("Horn", &[ai("aid:honk"), ai("aid:patience_min"), ai("aid:patience_max"), ai("aid:honk_repeat_min"), ai("aid:honk_repeat_max"), ai("aid:honk_max"), ai("aid:angry"), ai("aid:angry_decel")]),
            sec("Driver mistakes", &[
                it("aid:flaws", "Drivers make mistakes", "A few drivers forget to indicate or switch on their lights, some cars have a broken bulb"),
                ai("aid:no_indicator"),
                ai("aid:forget_indicator"),
                ai("aid:forget_chance"),
                ai("aid:forget_min"),
                ai("aid:forget_max"),
                ai("aid:no_lights"),
                ai("aid:no_lights_bright_min"),
                ai("aid:no_lights_bright_max"),
                ai("aid:rear_fog_misuse"),
                ai("aid:broken_bulb"),
            ]),
        ],
    },
    PageDef {
        title: "Map & HUD",
        takes_from: &[],
        sections: &[
            sec("Minimap", &[
                it("navigator", "Show the minimap", "The small map with your route and the next stop"),
                it("sel navigator_corner", "Minimap position", "Which corner of the screen it sits in"),
                it("nav_ai", "Traffic on the maps", "Shows the other vehicles on the minimap and the city map"),
                it("nav_arrows", "Route arrows on the road", "OMSI 2's arrows over the road that show the way"),
            ]),
            sec("On screen while driving", &[
                it("timetable_win", "Timetable", "The stops of your tour with their times (while you drive a tour)"),
                it("info_bar", "Info bar at the top", "Time, speed, temperature, passengers and the next stop in one line"),
                it("notes", "Driving hints", "Why the bus does not move, the change due, what a service did - top left"),
                it("fps", "Frame rate counter", "Frames per second in the top right corner"),
                it("tooltips", "Names of buttons under the mouse", "Says what the cursor points at in the cab"),
            ]),
            sec("Multiplayer", &[
                it("chat", "Chat", "The chat of an online game"),
                it("name_tags", "Players' names above their buses", "Shows who drives which bus"),
            ]),
        ],
    },
    PageDef {
        title: "Controls",
        takes_from: &["Controls"],
        sections: &[
            sec("Keyboard", &[
                it("sel drive_keys", "Driving keys", "Which keys accelerate, brake and steer"),
                it("steering_linear", "Steady steering keys", "The steering keys turn the wheel at OMSI's steady pace"),
                it("red_steer_spd", "Slower steering at speed", "The steering keys act slower the faster you go (OMSI's redSteerSpd)"),
                it("old_steering", "Wheel stays where you leave it", "The wheel does not centre itself: turn it back yourself"),
                it("brake_hold", "Brake key stays on", "The brake stays applied until you press the throttle"),
            ]),
            sec("Mouse", &[
                it("mouse", "Steer with the mouse", "The mouse steers, and controls the throttle and the brake"),
                it("mouse_sens", "Mouse steering sensitivity", "How far the wheel turns for a movement of the mouse"),
                it("mouse_smooth", "Smooth mouse steering", "The wheel follows the cursor smoothly; off: at once, as in OMSI"),
                it("mouse_right", "Right click stops mouse steering", "As in OMSI; off: the right button only looks round"),
            ]),
            sec("Steering wheel and pedals", &[
                it("ff", "Force feedback and vibration", "The wheel pushes back, controllers vibrate"),
                it("ff_invert", "Reverse force feedback", "For wheels that push the wrong way"),
                it("wheel_range", "Wheel rotation", "How far your steering wheel turns, lock to lock"),
                it("wheel_lock", "Full lock at", "How far you turn it for the bus's full lock"),
                it("pedal_t", "Throttle pedal strength", "How strongly the throttle pedal acts"),
                it("pedal_b", "Brake pedal strength", "How strongly the brake pedal acts"),
            ]),
            sec("Driving aids", &[
                it("auto_clutch", "Automatic clutch", "The clutch is worked for you"),
                it("auto_shift", "Automatic gear changes", "A manual gearbox's gears are changed for you"),
                it("momentary_gears", "Hold the gear buttons", "Letting go of a manual gear button returns to neutral"),
                it("blinker_cancel", "Indicators switch off after a turn", "Off: they stay on until you switch them off"),
            ]),
        ],
    },
    PageDef {
        title: "Camera",
        takes_from: &["Camera"],
        sections: &[
            sec("Driver's seat", &[
                it("fov", "Field of view", "How much you see at once"),
                it("seat 1", "Seat forward and back", "Move the driver's seat towards the windscreen or away"),
                it("seat 2", "Seat height", "Raise or lower the driver's seat"),
                it("seat 0", "Seat left and right", "Move the driver's seat sideways"),
                it("seat_reset", "Reset the seat", "Put the seat back where the bus has it"),
                it("hands_in_cab", "Driver's hands in view", "Shows your hands on the steering wheel"),
            ]),
            sec("Looking around", &[
                it("look_sens", "Mouse look sensitivity", "How fast the view turns with the mouse (100 % is OMSI's)"),
                it("alt_view", "Right mouse button turns the view", "Shift+right zooms; off: right zooms as in OMSI, the wheel button turns"),
                it("head", "Head movement", "Your head sways with braking, accelerating and bends"),
                it("steer_look", "Look into bends", "The view turns with the steering wheel"),
                it("steer_look_angle", "How far you look into bends", "The view's turn at full lock"),
                it("steer_look_response", "How quickly you look into bends", "How fast the view follows the steering"),
                it("headtrack", "Head tracking", ""),
            ]),
            sec("Outside views", &[
                it("cam_smooth", "Smooth camera changes", "The camera glides from one view to the next"),
                it("camcoll", "Camera stays out of objects", "The outside camera does not pass through walls and trees"),
                it("driver", "Driver at the wheel", "Shows the driver in the outside views and in the mirrors"),
            ]),
            sec("VR headset", &[
                it("vr", "Use a VR headset (OpenXR)", ""),
                it("sel vr_scale", "Picture sharpness", ""),
                it("sel vr_head_smoothing_ms", "Head movement smoothing", ""),
                it("sel vr_mirror_rate", "Mirror refresh rate", ""),
                it("vr_desktop_mirror", "Show the headset's picture on the monitor", ""),
            ]),
        ],
    },
    PageDef {
        title: "Graphics",
        takes_from: &["Graphics", "AI two-stroke smoke"],
        sections: &[
            sec("Quality", &[
                it("preset", "Quality preset", "Sets most of the graphics settings at once"),
                it("sel graphics", "Renderer", "Vanilla looks as OMSI 2 does; Enhanced adds modern lighting"),
                it("sel render_scale", "Render resolution", "Lower is faster, higher is sharper"),
                it("sel msaa", "Anti-aliasing", "Smooths jagged edges"),
                it("sel anisotropy", "Texture filtering", "Keeps roads and walls sharp at a slant"),
            ]),
            sec("Lighting and shadows", &[
                it("shadows", "Sun shadows", "Objects cast shadows in the sunlight"),
                it("sel shadow_size", "Shadow quality", "Sharper shadows cost more"),
                it("sel shadow_casters", "What casts shadows", ""),
                it("ssao", "Ambient occlusion", "Soft shade in corners and under things"),
                it("reflections", "Reflections", "Paint, chrome and glass reflect their surroundings"),
                it("clouds", "Clouds", "A sky with moving clouds"),
            ]),
            sec("Detail", &[
                it("detail_textures", "Fine detail up close", "The ground and large walls get a fine grain when you are near"),
                it("sel mirror_size", "Mirror quality", "How sharp the mirrors are"),
                it("led_glow", "LED display glow", "How strongly the dots of LED destination displays glow"),
                it("led_mips", "LED display smoothing", "How smooth LED displays look from a distance"),
            ]),
            sec("Smoke", &[
                it("exhaust", "Vehicle smoke", "How much exhaust, steam and spray vehicles make"),
                it("aid:smoke", "Trabant and Wartburg smoke", "Two-stroke cars leave a blue-grey cloud"),
                ai("aid:smokers"),
                ai("aid:smoke_base"),
                ai("aid:smoke_puff"),
                ai("aid:smoke_cold"),
                ai("aid:smoke_cold_time"),
                ai("aid:smoke_smoker"),
                ai("aid:smoke_density"),
            ]),
        ],
    },
    PageDef {
        title: "Display & performance",
        takes_from: &["Display and memory"],
        sections: &[
            sec("Window", &[
                it("fullscreen", "Fullscreen", "The game fills the whole screen"),
                it("sel resolution", "Window size", ""),
                it("triple_screen", "Three screens", "One picture over three monitors; a VR headset comes first"),
                it("triple_hud_center", "", ""),
                it("triple_span", "", ""),
                it("triple_width_mm", "", ""),
                it("triple_distance_mm", "", ""),
                it("triple_bezel_mm", "", ""),
                it("triple_left_angle_deg", "", ""),
                it("triple_right_angle_deg", "", ""),
                it("triple_eye_height_mm", "", ""),
            ]),
            sec("Frame rate", &[
                it("vsync", "V-sync", "Waits for the screen: no tearing, a little more delay"),
                it("sel max_fps", "Frame rate limit", "The most frames a second the game draws"),
            ]),
            sec("How far you see", &[
                it("sel view_distance", "View distance", "How far the map is loaded around you"),
                it("sel max_obj_dist", "Object distance", "How far away objects are still drawn"),
                it("sel min_obj_size", "Small objects", "Leave out small far objects to go faster"),
            ]),
            sec("Memory", &[
                it("sel texture_memory", "Texture memory", "How much of the graphics card the textures may use"),
                it("texture_compression", "Compress textures", "Uses less graphics memory, loads a little slower"),
                it("gfxprofile", "Load a graphics profile", "Applies a profile saved in the launcher"),
            ]),
        ],
    },
    PageDef {
        title: "Sound",
        takes_from: &["Sound"],
        sections: &[
            sec("Volume", &[
                it("volume", "Master volume", "How loud the whole game is"),
                it("vol_ai", "Traffic", "Other vehicles"),
                it("vol_scenery", "Surroundings", "Birds, trains, building sites and other sounds of the map"),
            ]),
            sec("Effects", &[it("doppler", "Doppler effect", "Approaching vehicles sound higher, receding ones lower")]),
        ],
    },
    PageDef {
        title: "Interface",
        takes_from: &["Interface"],
        sections: &[
            sec("Language", &[
                it("sel language", "Language", "The language of the menus and messages"),
                it("machine_translation", "Translate the rest automatically", "Translates texts nobody has translated, on this computer (a 620 MB download once)"),
            ]),
            sec("Size and look", &[
                it("ui_scale", "Interface size", "How large texts, menus, the timetable and the minimap are"),
                it("ui_scale_window", "Grow with the window", "On a window taller than 1080p the interface grows with it"),
                it("ui_opacity", "Background opacity", "How solid the backgrounds of the interface are"),
            ]),
            sec("Settings", &[
                it("advanced", "Show advanced settings", "Thresholds, timings and technical options too"),
                it("reset", "Reset all settings", "Everything but the language, the keys and the game folder goes back to how it came"),
            ]),
        ],
    },
];

/// The vehicle window.
pub const VEHICLE: &[PageDef] = &[
    PageDef {
        title: "Bus",
        takes_from: &["Display and driver"],
        sections: &[
            sec("Signs", &[
                it("dest", "Destination", "What the destination display shows"),
                it("number", "Fleet number", "The bus's own number"),
            ]),
            sec("Timetable and driver", &[
                it("hof", "Timetable file (HOF)", "Which depot file the timetable and the destinations come from"),
                it("driver", "Driver profile", "Who drives: the profile your km, punctuality and earnings go to"),
            ]),
        ],
    },
    PageDef {
        title: "Service",
        takes_from: &["Service"],
        sections: &[
            sec("At the depot", &[
                it("refuel", "Refuel", "Fills the tank"),
                it("wash", "Wash", "Cleans the bus"),
                it("repair", "Repair", "Mends what is worn or broken"),
            ]),
            sec("When something is wrong", &[
                it("reset", "Put back on its wheels", "Stands the bus upright again"),
                it("reload", "Reload the bus", "Reads the bus's files again and drives on from here (for modders)"),
            ]),
        ],
    },
    PageDef {
        title: "Vehicles",
        takes_from: &["Vehicles"],
        sections: &[
            sec("Drive another", &[
                it("switch", "Drive the next vehicle", "Take the wheel of another vehicle standing in the world"),
                it("swap", "Swap for another vehicle", "Another vehicle in this one's place, and you at its wheel"),
                it("place", "Place a vehicle", "Put a vehicle of your choice into the world"),
            ]),
            sec("Coupling", &[
                it("couple", "Couple", "Couple to the vehicle in front or behind"),
                it("uncouple", "Uncouple", "Separate the coupled vehicles"),
            ]),
            sec("On foot", &[it("getout", "Get out", "Step out of the bus and walk about")]),
            sec("Remove", &[
                it("remove", "Remove this vehicle", "Takes the vehicle you drive out of the world"),
                it("clearplaced", "Remove the placed vehicles", "Takes every vehicle you placed out of the world"),
            ]),
        ],
    },
    PageDef {
        title: "Go to",
        takes_from: &[],
        sections: &[sec("", &[
            it("teleport", "Somewhere on the map", "Pick a place on the city map and go there"),
            it("tplist", "A start point", "Go to one of the map's start points"),
        ])],
    },
];

/// The world window.
pub const WORLD: &[PageDef] = &[
    PageDef {
        title: "Time",
        takes_from: &["Time"],
        sections: &[
            sec("Clock", &[
                it("time_sync", "Real time", "The game's clock and date follow your computer's"),
                it("noop", "", ""),
                it("time_edit", "", ""),
                it("hour", "Hour", ""),
                it("minute", "Minute", ""),
                it("clock_ontime", "", ""),
            ]),
            sec("Speed", &[it("speed", "Time speed", "How fast the world's clock runs")]),
        ],
    },
    PageDef {
        title: "Weather",
        takes_from: &["Weather", "Temperature and wind"],
        sections: &[
            sec("Live weather", &[
                it("metar_sync", "Real weather (METAR)", "The weather follows a real airport's report"),
                it("metar_src", "Airport", "Which airport's report"),
                it("metar_icao_edit", "Airport code", "Type any airport's four-letter ICAO code"),
                it("metar_refresh", "Fetch the report now", "Gets the airport's latest weather without waiting"),
                it("metar_once", "Load the report once", "Takes the airport's weather once, without following it"),
            ]),
            sec("Weather now", &[
                it("weather", "Weather preset", "A ready-made weather"),
                it("weather_custom", "Change the weather by hand", "Keeps the weather now and lets you change it below"),
                it("cloudkind", "Clouds", "From a clear sky to overcast"),
                it("visibility", "Visibility", "How far you see; less is fog"),
                it("precipkind", "Rain or snow", "Whether it rains, snows or stays dry"),
                it("rain_amt", "How hard it rains or snows", "From a drizzle to a downpour"),
                it("wet", "Wet roads", "How wet the roads are now"),
            ]),
            sec("Temperature and wind", &[
                it("temp", "Temperature", "The air temperature outside"),
                it("wind_speed", "Wind speed", "It moves the clouds"),
                it("wind_dir", "Wind direction", "Where it blows from (0° is north)"),
            ]),
        ],
    },
    PageDef {
        title: "Traffic and passengers",
        takes_from: &["Traffic and people"],
        sections: &[sec("", &[
            it("traffic", "Vehicles on the road now", "How many vehicles drive about the map at the moment"),
            it("pax", "Passengers", "How many people wait at the stops and ride"),
        ])],
    },
    PageDef {
        title: "Tools",
        takes_from: &["Tools"],
        sections: &[sec("", &[it("editor", "Object editor", "Place and move objects in the world")])],
    },
];

/// A row's label with its name and description replaced where `item` says ("" keeps
/// them); a setting that needs a restart says so in the new description too.
fn relabel(label: &str, item: &Item) -> String {
    let mut p: Vec<String> = label.split('\u{1f}').map(str::to_string).collect();
    p.resize(5, String::new());
    if !item.name.is_empty() {
        p[0] = item.name.to_string();
    }
    let later = p[3] == LATER;
    if !item.desc.is_empty() {
        p[3] = if later { format!("{} ({})", item.desc, AFTER_RESTART.to_lowercase()) } else { item.desc.to_string() };
    } else if later {
        p[3] = AFTER_RESTART.to_string();
    }
    p.join("\u{1f}")
}

/// The pages `old` (as `options_pages` makes them) in the layout of `layout`: each page's
/// sections with their rows (a section without any left out), renamed; the rows the layout
/// does not place under "More" on the page that takes their old page; old pages no layout
/// page takes (the VR navigator's) after them as they were.
pub fn regroup(old: Vec<(&'static str, Rows)>, layout: &[PageDef]) -> Vec<(&'static str, Rows)> {
    let heading = crate::game_lists::HEADING;
    let taken: Vec<&str> = layout.iter().flat_map(|p| p.takes_from.iter().copied()).collect();
    // every row of the pages the layout takes, by its action, and where it was
    let mut rows: Vec<(&'static str, String, String)> = Vec::new();
    for (title, page) in &old {
        if !taken.contains(title) {
            continue;
        }
        for (label, id) in page {
            if id != heading {
                rows.push((title, label.clone(), id.clone()));
            }
        }
    }
    let placed: Vec<&str> = layout.iter().flat_map(|p| p.sections.iter()).flat_map(|s| s.items.iter()).map(|i| i.id).collect();
    let mut out: Vec<(&'static str, Rows)> = Vec::new();
    for page in layout {
        let mut list: Rows = Vec::new();
        for section in page.sections {
            // (every row of the action: a few information rows share one)
            let found: Rows = section.items.iter().flat_map(|item| rows.iter().filter(move |r| r.2 == item.id).map(move |r| (relabel(&r.1, item), r.2.clone()))).collect();
            if found.is_empty() {
                continue;
            }
            if !section.title.is_empty() {
                list.push((crate::game_lists::row(section.title, 'i', "", "", None), heading.to_string()));
            }
            list.extend(found);
        }
        let more: Rows = rows.iter().filter(|r| page.takes_from.contains(&r.0) && !placed.contains(&r.2.as_str())).map(|r| (r.1.clone(), r.2.clone())).collect();
        if !more.is_empty() {
            list.push((crate::game_lists::row("More", 'i', "", "", None), heading.to_string()));
            list.extend(more);
        }
        if !list.is_empty() {
            out.push((page.title, list));
        }
    }
    for (title, page) in old {
        if !taken.contains(&title) {
            out.push((title, page));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(name: &str, desc: &str, id: &str) -> (String, String) {
        (crate::game_lists::row(name, 's', "on", desc, None), id.to_string())
    }

    #[test]
    fn rows_go_to_their_new_page_renamed_and_nothing_is_lost() {
        let old: Vec<(&'static str, Rows)> = vec![
            ("Gameplay", vec![r("Navigator", "Enables/Disables the Minimap", "navigator"), r("Collisions with vehicles", "", "coll_vehicles"), r("Something new", "", "new_thing")]),
            ("Sound", vec![r("Doppler effect", "", "doppler")]),
            ("VR", vec![r("Navigator", "", "vr_nav_x")]),
        ];
        let pages = regroup(old, OPTIONS);
        let titles: Vec<&str> = pages.iter().map(|p| p.0).collect();
        assert_eq!(titles, ["Driving", "Map & HUD", "Sound", "VR"]);
        let driving = &pages[0].1;
        // a section heading, the renamed row, then what the layout does not know under More
        assert!(driving[1].0.starts_with("Crash into other vehicles"));
        assert!(driving.iter().any(|x| x.1 == "new_thing"));
        assert!(driving[driving.len() - 2].0.starts_with("More"));
        let map = &pages[1].1;
        assert!(map[1].0.starts_with("Show the minimap\u{1f}s\u{1f}on\u{1f}The small map"));
    }

    #[test]
    fn a_setting_needing_a_restart_still_says_so() {
        let row = crate::game_lists::row("Maintenance", 'o', "Normal", LATER, None);
        let item = it("sel maintenance", "Vehicle wear", "How quickly parts wear out");
        assert!(relabel(&row, &item).contains("How quickly parts wear out (applies after a restart)"));
        let plain = it("sel resolution", "Window size", "");
        assert!(relabel(&crate::game_lists::row("Window size", 'o', "Auto", LATER, None), &plain).contains(AFTER_RESTART));
    }

    #[test]
    fn every_ai_setting_has_its_place() {
        let placed: Vec<&str> = OPTIONS.iter().flat_map(|p| p.sections.iter()).flat_map(|s| s.items.iter()).map(|i| i.id).collect();
        for n in crate::ai_drivers::Config::NAMES {
            assert!(placed.contains(&format!("aid:{n}").as_str()), "aid:{n} has no place in the layout");
        }
    }
}
