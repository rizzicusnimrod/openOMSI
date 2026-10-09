# TH_Wald multiplayer timetable: four tours on a two-hour cycle; MP 1-3 meet at
# Lichtentanne ZOB (stands A, B, C) at every even hour, when MP 4 (Rennsteigbus) leaves
# Wurzbach Markt. Usage: th_wald_trips.py trips.json (lists the trips), then
# make_th_wald_multiplayer.py trips.json addons/TH_Wald/maps/TH_Wald/TTData/Multiplayer.ttl
import json, sys
trips = json.load(open(sys.argv[1]))
CYCLES = range(4 * 60, 23 * 60, 120)   # cycle starts 4:00 ... 22:00
TOURS = [
    ("MP 1 - 731 Wurzbach + Liebschuetz", [(0, "731_ZOB_WBM"), (45, "731_WBM_ZOBA"), (85, "731_ZOB_LBS"), (104, "731_LBS_ZOBA")]),
    ("MP 2 - 732 + 739 Bad Lemnitz + Lichtenhain", [(0, "732_ZOB_BLB"), (36, "739_BLB_LHS"), (60, "739_LHS_BLB"), (85, "732_BLB_ZOBB")]),
    ("MP 3 - 735 Altenfeld", [(0, "735_ZOB_AFB"), (30, "735_AFB_ZOBC"), (60, "735_ZOB_AFB"), (90, "735_AFB_ZOBC")]),
    ("MP 4 - 750 Rennsteigbus Wurzbach - Bad Lemnitz", [(0, "RWB_14_WBM_BLB"), (62, "RWB_13_BLB_WBM")]),
]
def end_place(n): return trips[n]["stations"][-1]["name"]
def start_place(n): return trips[n]["stations"][0]["name"]
def dur(n): return trips[n]["profiles"][0]["dur"]
out = ["-----------------------", "Time Table Line File", "-----------------------", "",
       "Multiplayer timetable for 3-4 players (openOMSI fork): MP 1-3 meet at Lichtentanne ZOB",
       "(stands A, B, C) at every even hour, when MP 4 leaves Wurzbach Markt.",
       "No AI bus drives these tours (AI group 'Players only').", "", "[userallowed]", "", "[priority]", "1"]
for name, pattern in TOURS:
    seq = [(c + off, trip) for c in CYCLES for off, trip in pattern]
    # each trip must start where the one before ended (the stop, any stand of it) and
    # after it arrived
    for (t0, a), (t1, b) in zip(seq, seq[1:]):
        stop = lambda s: s.split(" St.")[0].split(" Stg.")[0].split(", Burgbr.")[0]
        assert t0 + dur(a) <= t1 - 3, (name, a, b, t0, t1)
        assert stop(end_place(a)).replace("Lichtent.", "Lichtentanne") [:12] == stop(start_place(b)).replace("Lichtent.", "Lichtentanne")[:12], (a, end_place(a), b, start_place(b))
    out += ["------------------------------------", "", "[newtour]", name, "Players only", "1023", "",
            "------------------------------------", ""]
    for t, trip in seq:
        out += [f"  Dep.: {t // 60}:{t % 60}:0", "[addtrip]", trip, "0", f"{t:.3f}", ""]
    print(f"{name}: {len(seq)} trips, first {seq[0][0]//60}:{seq[0][0]%60:02d} {seq[0][1]}, last ends {int(seq[-1][0]+dur(seq[-1][1]))//60}:{int(seq[-1][0]+dur(seq[-1][1]))%60:02d}")
open(sys.argv[2], "w", encoding="cp1252", newline="").write("\r\n".join(out) + "\r\n")
