# TH_Wald multiplayer timetables, made of the map's own trips:
#
# Multiplayer.ttl - up to 8 players, each on a route of their own. Group A (MP 1-4) runs a
# two-hour cycle from every even hour: MP 1-3 leave Lichtentanne ZOB (stands A, B, C)
# together, when MP 4 (Rennsteigbus) leaves Wurzbach Markt. Group B (MP 5, 6, 8) is the same
# an hour later, from every odd hour. MP 3 is at ZOB every hour, MP 7 runs 735 at half past
# (and meets MP 1/5 at ZOB at about twenty past).
#
# Multiplayer 735.ttl - 3 players on line 735 together: a bus every 20 minutes, each going
# Lichtentanne ZOB - Altenfeld, Bahnhof and back in an hour (from ZOB at :05, :25 and :45 -
# the minutes the map's own buses stand least at those stops). Not with MP 3/7 in one session:
# they share the stands.
#
# In each line no two tours stand at one stop stand at the same time (checked below).
# Usage: th_wald_trips.py trips.json (lists the trips), then
# make_th_wald_multiplayer.py trips.json addons/TH_Wald/maps/TH_Wald/TTData
import json, os, sys
trips = json.load(open(sys.argv[1]))
EVEN = range(4 * 60, 23 * 60, 120)    # cycles 4:00 ... 22:00
ODD = range(5 * 60, 22 * 60, 120)     # cycles 5:00 ... 21:00
HOURLY = range(4 * 60, 23 * 60, 60)   # cycles 4:00 ... 22:00
P731 = [(0, "731_ZOB_WBM"), (45, "731_WBM_ZOBA"), (85, "731_ZOB_LBS"), (104, "731_LBS_ZOBA")]
P732 = [(0, "732_ZOB_BLB"), (36, "739_BLB_LHS"), (60, "739_LHS_BLB"), (85, "732_BLB_ZOBB")]
P750 = [(0, "RWB_14_WBM_BLB"), (62, "RWB_13_BLB_WBM")]
def p735(at): return [(at, "735_ZOB_AFB"), (at + 30, "735_AFB_ZOBC")]
LINES = {
    "Multiplayer.ttl": (
        ["Multiplayer timetable for up to 8 players (openOMSI fork). MP 1-3 leave Lichtentanne ZOB",
         "(stands A, B, C) together at every even hour, when MP 4 leaves Wurzbach Markt; MP 5, 6",
         "and 8 the same at every odd hour. MP 3 is at ZOB every hour, MP 7 runs 735 at half past."],
        [
            # (the names of MP 1-4 stay as in the first version: players with either file match)
            ("MP 1 - 731 Wurzbach + Liebschuetz", EVEN, P731),
            ("MP 2 - 732 + 739 Bad Lemnitz + Lichtenhain", EVEN, P732),
            ("MP 3 - 735 Altenfeld", range(4 * 60, 24 * 60, 60), p735(0)),
            ("MP 4 - 750 Rennsteigbus Wurzbach - Bad Lemnitz", EVEN, P750),
            ("MP 5 - 731 Wurzbach + Liebschuetz (odd hours)", ODD, P731),
            ("MP 6 - 732 + 739 Bad Lemnitz + Lichtenhain (odd hours)", ODD, P732),
            ("MP 7 - 735 Altenfeld (half past)", HOURLY, p735(30)),
            ("MP 8 - 750 Rennsteigbus Wurzbach - Bad Lemnitz (odd hours)", ODD, P750),
        ],
    ),
    "Multiplayer 735.ttl": (
        ["Multiplayer timetable for 3 players on line 735 (openOMSI fork): a bus every 20 minutes",
         "between Lichtentanne ZOB and Altenfeld, Bahnhof. Bus 1 leaves ZOB at :05, bus 2 at :25,",
         "bus 3 at :45, and Altenfeld half an hour later. Not together with MP 3/7 of the line",
         "Multiplayer in one session (the same stands)."],
        [
            ("735 Bus 1 - from ZOB at 05", HOURLY, p735(5)),
            ("735 Bus 2 - from ZOB at 25", HOURLY, p735(25)),
            ("735 Bus 3 - from ZOB at 45", HOURLY, p735(45)),
        ],
    ),
}
def first(n): return trips[n]["stations"][0]["name"]
def last(n): return trips[n]["stations"][-1]["name"]
def dur(n): return trips[n]["profiles"][0]["dur"]
def stop(s): return s.split(" St.")[0].split(" Stg.")[0].split(", Burgbr.")[0].replace("Lichtent.", "Lichtentanne")[:12]
for file, (about, tours) in LINES.items():
    out = ["-----------------------", "Time Table Line File", "-----------------------", ""] + about + [
        "No AI bus drives these tours (AI group 'Players only').", "", "[userallowed]", "", "[priority]", "1"]
    # who stands at which stop stand when: (stand, from, to, tour) - from arriving (five
    # minutes before leaving, at a tour's start) until leaving (five minutes after the last
    # arrival)
    stands = []
    print(file)
    for name, cycles, pattern in tours:
        seq = [(c + off, trip) for c in cycles for off, trip in pattern]
        # each trip starts where the one before ended (the stop, any stand of it), three
        # minutes after it arrived at the least
        for (t0, a), (t1, b) in zip(seq, seq[1:]):
            assert t0 + dur(a) <= t1 - 3, (name, a, b, t0, t1)
            assert stop(last(a)) == stop(first(b)), (a, last(a), b, first(b))
        for i, (t, trip) in enumerate(seq):
            arrived = seq[i - 1][0] + dur(seq[i - 1][1]) if i else t - 5
            stands.append((first(trip), arrived, t, name))
        t, trip = seq[-1]
        stands.append((last(trip), t + dur(trip), t + dur(trip) + 5, name))
        out += ["------------------------------------", "", "[newtour]", name, "Players only", "1023", "",
                "------------------------------------", ""]
        for t, trip in seq:
            out += [f"  Dep.: {t // 60}:{t % 60}:0", "[addtrip]", trip, "0", f"{t:.3f}", ""]
        end = seq[-1][0] + dur(seq[-1][1])
        print(f"  {name}: {len(seq)} trips, {seq[0][0] // 60}:{seq[0][0] % 60:02d} from {first(seq[0][1])}, ends {int(end) // 60}:{int(end) % 60:02d}")
    clashes = [(a, b) for i, a in enumerate(stands) for b in stands[i + 1:]
               if a[0] == b[0] and a[3] != b[3] and a[1] < b[2] and b[1] < a[2]]
    for a, b in clashes:
        print("CLASH", a, b)
    assert not clashes, f"{file}: {len(clashes)} stand clashes"
    open(os.path.join(sys.argv[2], file), "w", encoding="cp1252", newline="").write("\r\n".join(out) + "\r\n")
