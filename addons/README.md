# Add-ons

Files for OMSI 2's own folder, released beside the game by `.github/workflows/custom-release.yml`.

## TH_Wald

Realistic random traffic for the Thüringer Wald map. Unzip `TH_Wald-traffic-addon.zip` into the
OMSI 2 folder; it replaces these files in `maps\TH_Wald` (back them up first to go back):

- `unsched_trafficdens.txt` - traffic by the hour, from the BASt hourly counts of 2003 on
  Thuringia's Bundesstraßen (30 stations). 1.0 is the Monday-to-Thursday 16-17 h peak; Fridays,
  Saturdays, Sundays and the school holidays have curves of their own. The Friday and
  school-holiday curves use `[set_day_of_week]` 8, 16 and 24, which only this fork reads.
- `Holidays.txt`, `Holidays_DEU.txt`, `Holidays_ENG.txt` - the map's holidays plus the public
  holidays of 1995-2003 and Thuringia's school holidays (from the KMK lists).

## Budapest 181

Realistic random traffic for the Budapest 181 map (József Attila ltp., Budapest IX). Unzip
`Budapest181-traffic-addon.zip` into the OMSI 2 folder; it adds
`maps\Budapest 181\unsched_trafficdens.txt` (the map has none, and its global.cfg keeps the road
traffic at 1.0 day and night):

- Monday to Thursday as the Hungarian noise mapping rules have a large city's roads (80.4 % of
  the cars 6-18 h, 13.5 % 18-22 h, 6.1 % 22-6 h), with the sharp morning peak and softer afternoon
  peak of BKK's Budapest counts. 1.0 is the 7-8 h peak. Fridays, Saturdays, Sundays (and public
  holidays) and the school holidays (the map's Holidays.txt) have curves of their own; the Friday
  and school-holiday ones (`[set_day_of_week]` 8, 16, 24) only this fork reads.
- Made by `tools/traffic/make_budapest.py`.
