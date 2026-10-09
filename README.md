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
  (`TTData\Multiplayer.ttl`): four tours for playing together, each on its own route, every two
  hours from 4:00 to 24:00. MP 1 (731 Wurzbach), MP 2 (732/739 Bad Lemnitz, Lichtenhain) and MP 3
  (735 Altenfeld) leave Lichtentanne ZOB together at every even hour, from stands A, B and C;
  MP 4 (750 Rennsteigbus) leaves Wurzbach Markt at the same time. Start the session at an even
  hour (for example 7:58), and each player picks one MP tour with the automatic start place. No AI
  bus drives these tours.
* `Budapest181-traffic-addon.zip` - Budapest 181 traffic by the hour, weekday and school holidays.

**Updates:** this version does not update itself (the official openOMSI updater is switched off
so it can't replace it with the plain version). Check
[Releases](https://github.com/rizzicusnimrod/openOMSI/releases/latest) for new versions.

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
  car ahead on a wet road wets it too.
* No light column over low beams in rain, softer lamp-lit fog, and the weather's own cloud
  picture on the sky.

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

* **Realistic steering forces for wheels:** the force feedback is worked out from the front axle
  like a real bus - tyre load, caster and trail, grip running out, the power steering taking
  most of it away at speed, heavy with the engine off. Switch *Realistic steering forces* off in
  the launcher's wheel settings for the classic centring spring.
* A real tyre screech recording, and a vehicle smoke slider (Display settings).

### Photo mode and interface

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
* **Multiplayer: you don't meet the others** - everyone needs the host's map and the same
  version of this game.
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
