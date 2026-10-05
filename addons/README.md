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
