# Windscreen snow, cinematic headlamps, mirrors, Lichtentanne, snowy tyres: TDD evidence

**Source plan:** the `/plan` of 2026-10-23 evening (five reports: vibrating textures in TW
Lichtentanne, the windscreen's new snow not showing, blinding headlamps in the cinematic view at
night, mirrors without snow and too bright, snowy tyres), approved with "start working with all
phases", plus the user's addition during the work: "rain/snow on windshield needs to be more
obstructive to the visibility".
**Interpretation recorded:** the user's `game.log` showed the session started in *Autumn Gale* and
switched to *Light Snow* in-game, so "the snow on the windshield isn't visible" was read as "the
film does not follow a change of weather". The plan's suspected causes for Lichtentanne
(z-fighting of coplanar layers, as upstream #2603 fixed) were wrong: the repro found plant
detection by substring. The plan's planned depth prepass for enhanced mirrors was not needed:
measured, the enhanced mirrors cost 1.1-1.2 ms of GPU time more without one.
**Checkpoint commits:** none per stage - the fork is committed on the user's request only.

## User journeys

- As a driver whose weather turns to snow during a drive, I want the windscreen and windows to
  gather snow (and the wipers to clear it), not raindrops.
- As a video maker filming the bus at night with the cinematic camera, I want its headlamps to
  glow, not to blind the picture and black out everything round them.
- As a driver in the snow, I want the mirrors to show the same snow-covered roads and the same
  light as the windscreen.
- As a driver at Lichtentanne's bus station, I want the shelters, the kiosk and the railings to
  stand still and be drawn, not to sway, flicker or vanish.
- As a driver on a snowy road, I want my bus's and the traffic's tyres to pick up snow.
- As a driver in rain or snow, I want the film on the glass to take away a real part of the view
  until the wipers clear it.

## Task report

| Task | Test target | RED | GREEN |
|---|---|---|---|
| The film's look no longer depends on the weather at load time (`scene::film_look`) | `-p omsi-app --lib film_look_tests` | runtime: 2 of 3 failed (`snow_picture: false` for a film loaded in the dry) | 3 passed |
| Snowfall makes the glass snow, rain drops (`Lighting::glass_snow`, `lights::apply_weather`) | `-- glass_tests` | compile: no field `glass_snow` | passed |
| Film-camera headlamp dim (`lens_lamp_kept`, `LensFx::lamp_dim`) | `-p omsi-render --lib lens_lamp_tests` | runtime: 2 of 3 failed (stub kept 1.0) | 3 passed |
| The setting and the camera's lens (`cinematic_headlamps`, `Config::headlamps`, `look_fx`) | `-p omsi-app --lib the_film_camera_takes` | compile: 6 missing fields | passed |
| Matched mirrors (`enhanced_shading`, `RenderOptions::matched_mirrors`) | `-- shading_tests` | runtime: failed (stub ignored the option) | passed |
| Plain mirrors only dimmed at night (`camera_util::mirror_light_scale`) | `-- light_tests` | compile: missing settings fields | passed |
| Setting round trip, game and launcher (`mirror_matched`) | settings test, `-p omsi-launcher-core` | runtime (launcher: null), compile (app) | launcher 45, app 691 passed |
| Plant words by whole words, place names, vehicles (`retro::Rules::matches_words`, `foliage::Places`, `station_names`, `never_a_plant`) | `-- foliage` | runtime: 4 of 4 new tests failed on stubs | 10 passed |
| Tyre snow model (`omsi_sim::tyre_snow::step`) | `-p omsi-sim --lib tyre_snow` | runtime: 3 of 5 failed (stub kept the coat) | 5 passed |
| Film obstruction (`glass_film`: `RAIN_HAZE`, `RAIN_DROP_DENSITY`, `GLASS_SNOW_GAIN`, `GLASS_SNOW_VEIL`) | `-p omsi-render --lib glass_film` | runtime: 3 of 3 failed (no haze, constants missing in WGSL) | 3 passed (one assertion corrected: a drop pixel is 0.953 opaque with the haze, not 0.94 exactly) |

The renderer-side wiring of the tyre snow (`Instance::coat`, `set_coat`, the `local` varying,
`tyre_snow` in both fragment paths) has no unit test of its own: it was checked by renders and by
the shader validation of `-p omsi-render`.

## Test specification

| # | What is guaranteed | Test | Type | Result |
|---|---|---|---|---|
| 1 | A bus loaded in the dry carries the snow picture in its rain films; the weather at load no longer decides | `scene::film_look_tests` (3) | unit | PASS |
| 2 | Snow falling turns the films to crystals, rain to drops | `lights::glass_tests` | unit | PASS |
| 3 | A white lamp facing a film camera is taken down by the setting; red lamps, side views and no setting keep all | `lens_lamp_tests` (3) | unit | PASS |
| 4 | The cinematic camera takes the setting (default 35 % kept), the dashcam does not | `cinema::tests::the_film_camera_takes_the_headlamps_down_by_the_setting` | unit | PASS |
| 5 | Mirrors are shaded as the window when matched, plainly when not | `shading_tests` | unit | PASS |
| 6 | Only a plain mirror is dimmed at night | `camera_util::light_tests` | unit | PASS |
| 7 | `mirror_matched` defaults on and survives a save in the game and in the launcher | `settings` / launcher-core tests | unit | PASS |
| 8 | `Verkehrszeichen`, `Reichelt`, `variety`, `streekbruecke` are no plants; `Laubbaum`, `Zierbaum`, `Blautanne`, `grass02` are | `foliage::tests::a_plant_word_inside_another_word_is_no_plant` | unit | PASS |
| 9 | The map's stop names (Lichtentanne) are no plants; a stop named after a plant alone keeps it | `the_maps_place_names_are_no_plants`, `the_stops_are_read_from_a_timetable` | unit | PASS |
| 10 | Parked vehicles are never plants | `a_vehicle_is_never_a_plant` | unit | PASS |
| 11 | Tyre snow: kept standing still, packed in ~100 m, less at speed, worn off on cleared roads (sooner when wet), frame-rate independent | `omsi_sim::tyre_snow::tests` (5) | unit | PASS |
| 12 | A soaked pane hides at least 20 % of the view between its drops; a snowed pane at least 45 %; WGSL and Rust constants agree | `glass_film::tests` (3) | unit | PASS |
| 13 | All shaders still validate (naga) with the new uniform field, varying and constants | `-p omsi-render --lib` | unit | PASS (119) |
| 14 | GPU pipelines build and draw | `-p omsi-render --lib -- --ignored` | GPU | 25 pass; `spv_dump` (needs `SPV_DUMP`) and `a_smoke_puff_fades_into_its_ground` fail - the latter fails identically on the unchanged tree (checked with `git stash`) |

Full runs at the end: omsi-app 695, omsi-render 119, omsi-sim 279, omsi-launcher-core 45,
omsi-texture 49, omsi-cfg 23, omsi-content 18, omsi-model 11 - all passed.

## Renders (offscreen, `D:\dev\vistest`)

- Windscreen: `g1b_cab` (old, bus loaded in snow) / `g2n_cab`, `g3_noon` (new, white crystals);
  `g2n_rain` (rain still drops); `g2n_vp` (Vanilla+). At night the dark specks over the lit road
  are the windscreen's dirt film (`S315UL_dirt.tga`), in the old build as well.
- Cinematic night *front* shot: `c0b_front_20.0` (old: two blinding discs), `c2_front_26.0` (new).
  The discs were the eye-glare veil (`OMSI_NO_GLARE` removed them), not the coronas.
- Mirrors: `m0_*_cmp`, `m1_cmp` (plain vs matched, noon, dusk, night).
- Lichtentanne ZOB (spawn 1554.7,805.8): `z4_cmp` (old: the kiosk missing in Enhanced, drawn in
  Vanilla+), `z6_cmp` (`OMSI_NO_WIND_SWAY`: drawn), `z7_kiosk`, `z7_seq` (new: moving frames
  without edge flicker). Cause found with `OMSI_DEBUG_FOLIAGE`: "Lichtentanne_ZOB_1.sco is a
  plant".
- Tyres: `t2_cmp`, `t3_cmp`, `t3_side` (30 m on Light Snow, coat 0.52).
- Film obstruction: `r1_cmp_rain`, `r1_cmp_snow`.

## Performance

Windowed bench (`D:\dev\perftest\benchw.sh`, TH_Wald terminus 2560x1440, Light Snow 12:00,
MSAA 8, GPU timers): 0.2.79 27.3 fps / this tree 26.6 fps; mirrors GPU 1.60 -> 1.95 ms, CPU
4.5 -> 5.3 ms (the matched mirrors). A separate pair of runs of this tree with plain vs matched
mirrors: 28.5 / 28.5 fps, mirrors GPU 0.77 / 1.91 ms.

## Known gaps

- The tyre snow's shader mask (rubber by darkness) can whiten a mod's black rims too.
- Upstream's own coplanar-overlay fix (#2603) is still not in the fork; nothing in this round
  needed it.
- The windscreen's snow seen from outside the bus at a distance is the film's texture as before.
