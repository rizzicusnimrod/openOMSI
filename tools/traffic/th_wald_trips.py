# TH_Wald trips (.ttp): name, line, first and last stop, profiles; JSON to argv[1].
import os, sys, re, json
D = r"C:\Program Files (x86)\Steam\steamapps\common\OMSI 2\maps\TH_Wald\TTData"
def lines(p):
    return open(p, encoding='cp1252').read().splitlines()
trips = {}
for f in sorted(os.listdir(D)):
    if not f.endswith('.ttp'): continue
    L = [l.strip() for l in lines(os.path.join(D, f))]
    i = 0; t = {'file': f, 'stations': [], 'profiles': []}
    while i < len(L):
        k = L[i]
        if k == '[trip]':
            t['name'] = L[i+1]; t['terminus'] = L[i+2]; t['line'] = L[i+3]; i += 4; continue
        if k == '[station]':
            t['stations'].append({'id': L[i+1], 'name': L[i+3]}); i += 9; continue
        if k == '[profile]':
            t['profiles'].append({'name': L[i+1], 'dur': float(L[i+2]), 'dep': {}}); i += 3; continue
        if k == '[profile_man_dep_time]' and t['profiles']:
            t['profiles'][-1]['dep'][int(L[i+1])] = float(L[i+2]); i += 3; continue
        i += 1
    trips[t.get('name', f[:-4])] = t
json.dump(trips, open(sys.argv[1], 'w'), indent=0)
for n, t in trips.items():
    if not t['stations']: continue
    pr = ' '.join(f"{p['name']}={p['dur']:g}" for p in t['profiles'])
    print(f"{n:24s} L{t['line']:4s} {t['stations'][0]['name'][:26]:26s} -> {t['stations'][-1]['name'][:26]:26s} n{len(t['stations']):2d} {pr}")
