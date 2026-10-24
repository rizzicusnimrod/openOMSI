<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/logos/openomsi-wordmark-light.svg">
    <img alt="openOMSI" src="assets/logos/openomsi-wordmark-dark.svg" width="420">
  </picture>
</p>

<p align="center"><b>custom edition</b></p>

<p align="center">
  <a href="https://github.com/rizzicusnimrod/openOMSI/releases/latest"><img alt="Version" src="https://img.shields.io/github/v/release/rizzicusnimrod/openOMSI?label=version&color=f47f30&style=for-the-badge"></a>
  <a href="https://github.com/rizzicusnimrod/openOMSI/releases"><img alt="Downloads" src="https://img.shields.io/github/downloads/rizzicusnimrod/openOMSI/total?style=for-the-badge&color=2d3138"></a>
  <a href="https://github.com/openOMSI-Project/openOMSI"><img alt="Based on openOMSI" src="https://img.shields.io/badge/based%20on-openOMSI-f47f30?style=for-the-badge"></a>
  <a href="LICENSE"><img alt="License" src="https://img.shields.io/badge/license-MIT-2d3138?style=for-the-badge"></a>
</p>

This is my own version of **[openOMSI](https://github.com/openOMSI-Project/openOMSI)**, the
from-scratch Rust recreation of the bus simulator **OMSI 2**. It keeps everything openOMSI does -
64-bit, multithreaded, a modern renderer, every OMSI 2 map, bus and mod - and adds a lot of its
own: realistic nights and rain, smarter AI drivers, traffic that follows the clock and the
calendar, force feedback that feels like a bus, a photo mode and a new-look interface.

> [!IMPORTANT]
> **You need an original copy of OMSI 2** (Steam or retail). openOMSI contains no game content:
> it plays the maps, buses and mods of your installed OMSI 2 and never writes into its folder.

> [!NOTE]
> This version is not made or supported by the openOMSI project. Please don't report its
> problems to them - tell me instead.


## Download and play

1. Download **`openOMSI-<version>-windows-x64.zip`** from
   [**Releases**](https://github.com/rizzicusnimrod/openOMSI/releases/latest).
2. Unzip it into a folder of its own that you can write to (Documents, a games folder - not
   `Program Files`).
3. Start **`openomsi.exe`**. Windows SmartScreen may warn about an unknown app: *More info* →
   *Run anyway*.
4. The launcher usually finds OMSI 2 by itself. If not, open **Setup** and choose the OMSI 2
   folder (Steam: `…\Steam\steamapps\common\OMSI 2`), then **Save**.
5. Pick a bus, a map and a duty on the **Drive** page and press **Start the duty**.

**Optional traffic add-ons** (also on the Releases page) - unzip into the OMSI 2 folder:

* `TH_Wald-traffic-addon.zip` - Thüringer Wald traffic by the hour, weekday and school holidays
  (1995-2003). It replaces `maps\TH_Wald\unsched_trafficdens.txt` and `Holidays*.txt`, so back
  those up first if you want to go back. It also adds the line **Multiplayer**
  (`TTData\Multiplayer.ttl`): eight tours for up to eight players, from 4:00 to 24:00, no two at
  the same stop stand at once. MP 1 (731 Wurzbach), MP 2 (732/739 Bad Lemnitz, Lichtenhain) and
  MP 3 (735 Altenfeld) leave Lichtentanne ZOB together at every even hour, from stands A, B and C;
  MP 4 (750 Rennsteigbus) leaves Wurzbach Markt at the same time. MP 5, 6 and 8 are MP 1, 2 and 4
  an hour later (every odd hour); MP 3 is at ZOB every hour, MP 7 runs 735 at half past. Start the
  session twenty minutes before an even hour (for example 7:40) with the host on MP 1-4, and each
  player picks one MP tour with the automatic start place: MP 1-4 begin at 8:00, MP 5 at 7:45 in
  Wurzbach, MP 6, 7 and 8 at 8:00-8:02 in Lichtenhain, Altenfeld and Bad Lemnitz. No AI bus
  drives these tours. The line **Multiplayer 735** (`TTData\Multiplayer 735.ttl`) puts three
  players on line 735 together, a bus every 20 minutes between Lichtentanne ZOB and Altenfeld:
  started at 8:00 (host on bus 1), bus 1 leaves ZOB at 8:05, bus 3 Altenfeld at 8:15, bus 2 ZOB
  at 8:25, and every hour after that. Don't mix it with MP 3/7 in one session (the same stops).
* `Budapest181-traffic-addon.zip` - Budapest 181 traffic by the hour, weekday and school holidays.

**Updates:** the launcher looks for new versions of this fork on
[Releases](https://github.com/rizzicusnimrod/openOMSI/releases/latest) and asks before it
installs one (**Update now** / **Not now**); your mods and settings stay. It never takes the
plain openOMSI's releases. *Settings → Install updates without asking* skips the question.

**Multiplayer:** everyone in a LAN session should run the same version from here.


## What's different from openOMSI

### Nights, light and weather

* **Realistic night driving:** ECE-pattern headlamps with a real cut-off, halogen-coloured lamps,
  reflective signs and delineator posts that shine back in your headlights (which signs do is
  editable in `retroreflective.cfg`).
* **Brighter towns at night:** street lamps reach further, and the eye adapts to lamp-lit
  streets the way it does in reality.
* **Lamps by day:** car head and tail lamps and traffic lights don't glare like stars from far
  away in daylight.
* **Windscreen rain with real wipers:** the water on your windscreen builds up drop by drop and
  each blade clears the band it sweeps; it comes back faster standing still, and spray from the
  car ahead on a wet road wets it too. On a soaked pane the water between the drops blurs the
  street into soft, wobbling shapes and street lamps into halos, so the wipers matter. In a
  snowfall the panes gather snow instead - flakes, clumps of it and melt water between them,
  also when the weather turns to snow during a drive - and the wipers sweep it off. How much
  it hides: *Rain and snow on the windscreen* (50-150 %) in the Graphics settings.
* No light column over low beams in rain, softer lamp-lit fog, and the weather's own cloud
  picture on the sky.

### Nature

* **Trees in the wind:** trees, bushes, hedges and grass sway with the weather's wind, gusts
  sweep across the land and leaves flutter (*Wind in the trees* in the Graphics settings).
  Which objects are plants is listed in `foliage.cfg`, like the reflective signs; a plant's
  word counts only at the start or the end of a word of a texture's name (a road sign's
  *Verkehrszeichen* holds no oak), the map's own places are no plants (Thüringer Wald's
  *Lichtentanne*) and neither are parked vehicles.
* **Trees through the year:** autumn colours from mid-September, bare crowns in winter and
  fresh green in spring, on maps that have no seasonal trees of their own (conifers stay
  green).
* **Lightning and thunder:** thunderstorms flash in the clouds and strike the ground, the
  street lit for an instant, and the thunder rolls in after it - later the farther away it
  struck.
* **Rainbows** opposite a low sun when it rains or showers are about, and short **contact
  shadows** where kerbs, posts and wheels meet the ground (Enhanced graphics).
* **Grass** grows on the verges and meadows and sways with the trees, fading out with the
  distance (*Grass* in the Graphics settings: Off, Sparse, Normal, Dense). Which ground
  textures grow it is listed in `grass.cfg`.
* **Snow:** a snow weather without snow on the road (`[snowOnRoad]`) leaves the roads cleared
  - dark and wet in a thaw, dry in a hard frost - with the plough's snow banked at the kerb;
  with it the roads lie under snow worn into ruts by the traffic (busier lanes wear more),
  pressed grey in a frost and churned to slush near 0 °C - sharp-edged and grainy close up,
  the tyres' lines running along them. Thin snow lies in patches, fresh snow glints here and
  there in the sun, and the falling flakes streak past as you drive (Enhanced
  graphics; *openOMSI snowy roads* in the Graphics settings switches back to the maps' own
  snowy road pictures). Driving on a snowy road packs snow into the tyres' treads - more
  the slower you go, worn off again on cleared roads - on every vehicle.

### Light and surfaces (Enhanced graphics)

* **Sun shafts** through the trees and between the houses, in fog and in a calm, damp
  morning's ground mist (*Sun shafts*: Off, Low, Medium, High).
* **Street reflections on vehicles:** bus paint, chrome and windows mirror the street and the
  cars round about, not only the sky.
* **Mirrors as the windscreen:** the mirrors show the same snow, light and colours as the view
  ahead (switch *Mirrors as the windscreen* off in the Graphics settings for the plainer, a
  little faster mirrors).
* **Sunlit cab:** the cab is lit through its windows, brighter on the sunny side, and the
  sun's patches move over the dash and the floor.
* **Relief on old textures:** cobbles, brick, stone and bark get a soft relief made from their
  own pictures up close (`relief.cfg` lists which).
* **Tyre tracks:** on a drying road the wheel lines dry first; in snow they are the ruts the
  traffic wears (see *Snow* above).
* **Temporal anti-aliasing** (off by default) steadies shimmering branches and wires, so a
  lower MSAA setting can look as smooth.

### AI traffic and drivers

* Drivers switch their lights on by the weather and the time, use hazards and the horn, flash
  their high beams when provoked or impatient (or dazzled by yours) and give a "go ahead" flash.
* Harder emergency braking with screeching tyres, and seeing trouble coming sooner.
* A firmer, brisker driving style through bends and junctions; junctions by the rule of the
  right are taken slowly, and **cars don't drive into a junction they can't leave** (no more
  blocked crossings).
* **Traffic by the clock and the calendar:** separate curves for weekdays, Fridays, weekends,
  public holidays and school holidays.
* Less CPU spent on AI traffic.

### Driving

* **Autopilot** (**Alt+A**, beta): it steers and drives the bus for you - it keeps to its lane,
  at junctions follows your duty's route (without one, the way your indicator points, else
  straight on), slows for speed limits and bends, keeps its distance to the traffic ahead, stops
  at red lights, gives way and pulls out of bus bays. Bus stops, doors and tickets stay yours:
  brake for a stop and the bus is yours again (so is it when you steer); Alt+A hands it back.
  The throttle drives faster while you hold it. **Alt+Q** / **Alt+E** top speed down / up,
  **Alt+D** the distance it keeps, **Alt+C** calm, normal or brisk; more on the *Driving* page
  of the options. Still rough: tight turns are not checked against kerbs, and articulated buses
  only roughly turn like one.
* **Realistic steering forces for wheels:** the force feedback is worked out from the front axle
  like a real bus - tyre load, caster and trail, grip running out, the power steering taking
  most of it away at speed, heavy with the engine off. Switch *Realistic steering forces* off in
  the launcher's wheel settings for the classic centring spring.
* A real tyre screech recording, and a vehicle smoke slider (Display settings).

### Cameras, photo mode and interface

* **Cinematic camera** (**Ctrl+F3**) for showcase and YouTube videos: an automatic director that
  cuts between chase, tracking, roadside fly-by, long-lens, drone, front, wheel-mount, crane,
  over-the-shoulder and bird's-eye shots, each placed where nothing blocks the view. Film looks
  (teal and orange, warm film, black and white...), letterbox, focus on the bus with depth of
  field, motion blur, anamorphic streaks and sun flares. **Ctrl+F3** again cuts to the next shot,
  **Ctrl+F2** holds the shot. You keep driving while it films. At night the headlamps shining
  into it are taken down so they don't blind the shot (*Headlamps into the camera*).
* **Dashcam view** (**Ctrl+F1**): a camera at the top of the windscreen with a fisheye lens,
  colour fringes, sensor noise (lots of it at night), video compression smear, rolling-shutter
  lean and wobble, mount shake over bumps, stars round lamps, and the date, time and speed in the
  picture. Both are set up on the *Cameras* page of the options.
* **Photo mode** (**F10**, or *Photo mode* in the game menu): a free camera, a real lens
  (12-300 mm, aperture, focus and depth of field), exposure and colour looks, and supersampled
  pictures up to 8192 px saved to `Screenshots\Photos`. In multiplayer it works too - the game
  just doesn't pause for anyone.
* **Ctrl+H hides the whole interface** for a clean view (change the key in the key settings).
* A new game menu, settings in a drawer with search, pages by what you want to do, an advanced
  settings switch, a modern minimap and a players list in multiplayer.

### Multiplayer (LAN)

* The other players' buses complete: their displays, script screens and every script variable.
* The host's AI traffic as its drivers show it, gliding smoothly along its curves.
* AI drivers react to every player's high beams, not just the host's.
* **Content for everyone, when you say yes:** joining asks whether to download what the host's
  session uses that you lack (its map, the players' buses, their objects and sounds). A player
  in a bus you don't have is shown in a stand-in, and you're asked whether to download that bus;
  the host is asked too when a player brings a bus it lacks, and passes it on to the others.
  Files are checked and nothing that runs code is taken.
* Nothing of the OMSI 2 folder is ever passed on (it holds the paid add-ons). A free mod you
  installed into it can be shared by listing its folder in `C:\Users\<you>\.openomsi\lan-share.cfg`,
  one a line (e.g. `Vehicles/MAN_NL202`).


## Photo mode keys

| Key | What it does |
| --- | --- |
| F10 / Esc | enter / leave photo mode |
| H | show or hide the panel |
| F12 | take the photo |
| F | focus on the middle of the picture |
| `[` `]` | tilt the camera |
| `-` `=` | zoom out / in (focal length) |


## When something goes wrong

* **"The original OMSI 2 was not found"** - choose the OMSI 2 folder under Setup; the message
  says what the chosen folder lacks.
* **The game closes after a few seconds, or "the graphics device was lost"** - update the
  graphics driver (NVIDIA, AMD or Intel's own). You can also try Settings → Graphics → Graphics
  API → DirectX 12.
* **Stuck at a bridge or an invisible wall** on a mod map: Esc → Options → *Collisions with
  objects*.
* **Multiplayer: you don't meet the others** - everyone needs the host's map (say yes when
  joining asks to download it) and the same version of this game.
* **Anything else:** the logs are in `C:\Users\<you>\.openomsi` (`game.log` for the last game).
  Send me that file with what happened.


## Building from source

```sh
git clone https://github.com/rizzicusnimrod/openOMSI.git && cd openOMSI
scripts\build-windows.cmd     # → dist\windows\openomsi.exe
```

Needs the latest [Rust stable](https://rustup.rs) and Visual Studio 2022's C++ tools; details in
[docs/BUILDING.md](docs/BUILDING.md). The default branch is `custom`; it takes in the official
openOMSI updates from time to time, keeping everything above.

The openOMSI documentation in [`docs/`](docs) applies here too: the
[user guide](docs/USER_GUIDE.md), [modding](docs/MODDING.md), [VR](docs/VR.md),
[plugins](docs/PLUGINS.md), [content formats](docs/FORMATS.md) and more.


## Credits and license

Built on [openOMSI](https://github.com/openOMSI-Project/openOMSI) by the openOMSI Project and its
contributors - all the hard work of recreating OMSI 2 is theirs. If you like the base game,
support them there.

Released under the [MIT License](LICENSE), like openOMSI. OMSI and OMSI 2 are trademarks of their
respective owners; this project is not affiliated with them.
