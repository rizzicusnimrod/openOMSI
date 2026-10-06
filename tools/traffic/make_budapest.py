"""Write unsched_trafficdens.txt for the map Budapest 181 (Jozsef Attila housing estate,
Budapest IX: Ulloi ut, Hatar ut, the M5 feeder, Ecseri ut).

    python tools/traffic/make_budapest.py OUT [GROUP]

There are no public hourly counts of these roads, so the curves are built on what is known:

- The Hungarian noise mapping rules (25/2004 KvVM, from the national road database OKA) put
  a car's day on a road inside a large city at 80.4 % between 6 and 18 h, 13.5 % between
  18 and 22 h and 6.1 % between 22 and 6 h (forgalomjelleg 3, evening and night at most
  21 %). The Monday to Thursday curve keeps exactly those shares.
- BKK's loop counts (bkk.hu, traffic data): a Budapest weekday has a sharper morning peak
  after a quick rise and a weaker afternoon peak after a strong middle of the day; Budapest's
  rush hours are 7-9 h and 15-18 h. The estate's roads are radials into town (Ulloi ut, the
  M5): in to work in the morning, out in the afternoon.
- Between holidays BKK measured some 20 % less traffic and 60 % less in the morning peak;
  the school holidays (the map's Holidays.txt) are taken as 10 % less, most of it the
  morning's school runs.
- Weekend levels and shapes as on European urban arterials: Saturday about three quarters
  of a weekday with a late-morning shopping peak, Sunday under three fifths with the
  evening's way home.

1.0 = the busiest hour of a Monday to Thursday (7-8 h): the game's traffic count is the
number of cars then. Each curve has a point in the middle of every hour.
"""
import sys

# share of the day's traffic per hour (h = h:00-h:59), % - each day type its own shape
MON_THU = [0.6, 0.35, 0.25, 0.25, 0.45, 1.3, 5.3, 8.0, 7.5, 6.3, 5.9, 6.0,
           6.2, 6.4, 6.9, 7.4, 7.3, 7.2, 5.2, 3.7, 2.6, 2.0, 1.8, 1.1]
FRI = [0.7, 0.4, 0.3, 0.3, 0.45, 1.2, 5.0, 7.3, 7.0, 6.1, 5.9, 6.1,
       6.4, 6.8, 7.4, 7.7, 7.5, 6.9, 5.5, 4.0, 2.9, 2.3, 2.0, 1.45]
SAT = [1.4, 0.9, 0.6, 0.5, 0.5, 0.8, 1.8, 3.0, 4.8, 6.4, 7.5, 7.9,
       7.7, 7.2, 6.6, 6.3, 6.2, 6.0, 5.6, 4.8, 3.8, 3.0, 2.4, 1.6]
SUN = [2.0, 1.3, 0.9, 0.7, 0.6, 0.8, 1.3, 2.0, 3.3, 4.8, 6.1, 6.8,
       6.8, 6.5, 6.4, 6.8, 7.2, 7.4, 7.0, 5.8, 4.6, 3.5, 2.4, 1.4]
# the school holidays: no school runs in the morning, a little less all day
SCHOOL = [1.0, 1.0, 1.0, 1.0, 1.0, 0.97, 0.9, 0.8, 0.84, 0.94, 0.97, 0.97,
          0.96, 0.96, 0.95, 0.93, 0.93, 0.94, 0.96, 0.98, 1.0, 1.0, 1.0, 1.0]

# a day's traffic against a Monday to Thursday
TOTAL = {"mon_thu": 1.0, "fri": 1.03, "sat": 0.76, "sun": 0.58, "mon_thu_school": 0.90, "fri_school": 0.93}

DAYS = [(1, "mon_thu", "Monday to Friday (OMSI 2; openOMSI: Monday to Thursday)"),
        (2, "sat", "Saturday"),
        (4, "sun", "Sunday, and public holidays in openOMSI"),
        (8, "fri", "Friday (openOMSI only, OMSI 2 passes it by)"),
        (16, "mon_thu_school", "Monday to Friday in the school holidays (openOMSI only)"),
        (24, "fri_school", "Friday in the school holidays (openOMSI only)")]


def volumes():
    """Each day type's hourly volume, in % of a Monday to Thursday's whole day."""
    def scaled(shape, total):
        s = sum(shape)
        return [v / s * 100.0 * total for v in shape]
    school = lambda base: [b * m for b, m in zip(base, SCHOOL)]
    return {
        "mon_thu": scaled(MON_THU, TOTAL["mon_thu"]),
        "fri": scaled(FRI, TOTAL["fri"]),
        "sat": scaled(SAT, TOTAL["sat"]),
        "sun": scaled(SUN, TOTAL["sun"]),
        "mon_thu_school": scaled(school(MON_THU), TOTAL["mon_thu_school"]),
        "fri_school": scaled(school(FRI), TOTAL["fri_school"]),
    }


def check():
    w = MON_THU
    day, evening, night = sum(w[6:18]), sum(w[18:22]), sum(w[22:]) + sum(w[:6])
    assert abs(sum(w) - 100.0) < 1e-6, sum(w)
    assert (round(day, 1), round(evening, 1), round(night, 1)) == (80.4, 13.5, 6.1), (day, evening, night)


def main():
    check()
    out_path = sys.argv[1]
    group = sys.argv[2] if len(sys.argv) > 2 else "Szemelyautok"
    vol = volumes()
    peak = max(vol["mon_thu"])
    lines = [
        "Random traffic by the hour for Budapest 181 (Jozsef Attila ltp., Budapest IX).",
        "Monday to Thursday as the Hungarian noise mapping rules have a large city's roads",
        "(80.4 % of the cars 6-18 h, 13.5 % 18-22 h, 6.1 % 22-6 h), shaped as BKK's counts",
        "describe a Budapest weekday: a sharp morning peak, a strong day, a softer afternoon peak.",
        "1.0 = the busiest hour of a Monday to Thursday (7-8 h).",
        "Made by make_budapest.py.",
        "",
        "=" * 60,
        "",
        "[group]",
        group,
        "1.0000",
        "",
    ]
    for mask, key, title in DAYS:
        v = [x / peak for x in vol[key]]
        edge = (v[23] + v[0]) / 2
        pts = [(0.0, edge)] + [(h + 0.5, v[h]) for h in range(24)] + [(24.0, edge)]
        lines += [title, "", "[set_day_of_week]", str(mask), ""]
        for t, x in pts:
            lines += ["[trafficdensity]", f"{t:.3f}", f"{x:.3f}", ""]
        day_sum = sum(vol[key])
        print(f"{key:16s} day {day_sum:6.1f} % of Mon-Thu, peak {max(v):.2f} at {v.index(max(v))}-{v.index(max(v)) + 1} h")
    with open(out_path, "w", encoding="cp1252", newline="\r\n") as f:
        f.write("\n".join(lines) + "\n")


if __name__ == "__main__":
    main()
