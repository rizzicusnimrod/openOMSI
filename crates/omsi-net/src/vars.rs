//! Every script variable of a player's vehicle, for the copies the others draw (`VARS`).
//!
//! The pose (`wire.rs`) carries what every copy needs at the full rate: where the bus is, its
//! lamps, switches and the values its sounds and moving parts follow. Everything else the
//! scripts work out - a gearbox's state, a display's mode, an IBIS, a ticket printer, a
//! variable a plugin or a `setvar` set - went nowhere, and another player's bus showed what
//! its own copy of the scripts made of it. This sends all of them: ten times a second the
//! variables that changed since they were last sent, and all of them in turn in between (a
//! "key frame" a slice at a time), so a lost datagram is mended within seconds and a player
//! who joins late sees the whole state. Strings go the same way.
//!
//! A message names the vehicle's variable table by a hash of its variables' names (the
//! game's): a copy made from other files (another version of the bus) takes nothing.
//!
//! ```text
//! byte 0     0xB5 (no text message starts with it)
//! byte 1     protocol
//! bytes 2-5  sender's id (u32 LE)
//! bytes 6-9  table hash (u32 LE)
//! byte 10    kind: 0 floats by index, 1 a run of floats, 2 strings by index
//! then       0: count u16, (index u16, f32)*      1: first u16, count u16, f32*
//!            2: count u16, (index u16, length u16, UTF-8)*
//! ```
//! All numbers little-endian. Indices are the receiver's `VarId` / `StrVarId` for the same
//! table.

/// The first byte of a `VARS` datagram.
pub const VARS_MAGIC: u8 = 0xB5;
const HEADER: usize = 11;
/// What one datagram holds at most (within `MAX_DATAGRAM`, with room to spare).
const ROOM: usize = 1300;
/// How often changes go out (s), and how many ticks between key frame slices (a bus's
/// thousands of variables went round in five seconds, its strings a slice a second).
const EVERY: f32 = 0.1;
const KEY_EVERY: u32 = 3;
const STRINGS_KEY_EVERY: u32 = 3;
/// At most this many datagrams of changes a tick: one held some 200 values, and a bus on
/// the move changes more than that - a display's change waited its turn behind them.
const MAX_DELTAS: usize = 4;
/// A changed string goes this many times more on the following ticks: one lost datagram
/// left a display on its old text until the key frame came round.
const STRING_REPEATS: u8 = 2;
/// A string longer than this goes cut (a display's text, a path).
const MAX_STRING: usize = 255;

/// Variables of a player's vehicle as they came (`LanSession::take_vars`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct VarsIn {
    pub id: u32,
    pub table: u32,
    pub floats: Vec<(u16, f32)>,
    pub strings: Vec<(u16, String)>,
}

/// Our side: what was sent last, and where the key frame has got to.
#[derive(Debug, Default)]
pub struct VarSender {
    table: u32,
    floats: Vec<f32>,
    strings: Vec<String>,
    acc: f32,
    ticks: u32,
    delta_from: usize,
    key_float: usize,
    key_string: usize,
    /// Strings changed lately, by index, and how many times more each goes.
    recent: Vec<(usize, u8)>,
    /// A key frame in a burst (`burst`): the variables and strings still to go in it.
    burst_floats: usize,
    burst_strings: usize,
}

fn header(protocol: u8, id: u32, table: u32, kind: u8) -> Vec<u8> {
    let mut d = Vec::with_capacity(ROOM + HEADER);
    d.push(VARS_MAGIC);
    d.push(protocol);
    d.extend_from_slice(&id.to_le_bytes());
    d.extend_from_slice(&table.to_le_bytes());
    d.push(kind);
    d
}

fn same(a: f32, b: f32) -> bool {
    a.to_bits() == b.to_bits() || (a.is_nan() && b.is_nan()) || (a - b).abs() <= 1.0e-6 * a.abs().max(1.0)
}

/// A string as it goes (cut at a character boundary).
fn cut(s: &str) -> &str {
    if s.len() <= MAX_STRING {
        return s;
    }
    let mut n = MAX_STRING;
    while !s.is_char_boundary(n) {
        n -= 1;
    }
    &s[..n]
}

impl VarSender {
    /// Everything again at once, over the next ticks (a player has joined).
    pub fn burst(&mut self) {
        self.burst_floats = self.floats.len();
        self.burst_strings = self.strings.len();
    }

    /// Our vehicle's variables (`ids[k]` holds `values[k]`, the same for strings) for this
    /// frame: the datagrams due now (none most frames).
    #[allow(clippy::too_many_arguments)]
    pub fn tick(&mut self, protocol: u8, id: u32, table: u32, float_ids: &[u16], floats: &[f32], string_ids: &[u16], strings: &[String], dt: f32) -> Vec<Vec<u8>> {
        if table != self.table || self.floats.len() != floats.len() || self.strings.len() != strings.len() {
            // another vehicle (or another table): everything is new
            *self = VarSender { table, floats: vec![f32::NAN; floats.len()], strings: vec!["\u{0}".into(); strings.len()], acc: EVERY, ..VarSender::default() };
        }
        self.acc += dt;
        if self.acc < EVERY {
            return Vec::new();
        }
        self.acc = 0.0;
        self.ticks = self.ticks.wrapping_add(1);
        let mut out = Vec::new();
        let n = floats.len().min(float_ids.len());
        // what changed, from where the last datagram had to stop (as many datagrams as it
        // takes, up to `MAX_DELTAS`)
        if n > 0 {
            let mut looked = 0;
            for _ in 0..MAX_DELTAS {
                let mut d = header(protocol, id, table, 0);
                d.extend_from_slice(&0u16.to_le_bytes());
                let mut count = 0u16;
                let mut k = self.delta_from % n;
                while looked < n && d.len() + 6 <= ROOM {
                    if !same(floats[k], self.floats[k]) {
                        d.extend_from_slice(&float_ids[k].to_le_bytes());
                        d.extend_from_slice(&floats[k].to_le_bytes());
                        self.floats[k] = floats[k];
                        count += 1;
                    }
                    k = (k + 1) % n;
                    looked += 1;
                }
                self.delta_from = k;
                if count > 0 {
                    d[HEADER..HEADER + 2].copy_from_slice(&count.to_le_bytes());
                    out.push(d);
                }
                if looked >= n {
                    break;
                }
            }
        }
        // a slice of the key frame: a run of consecutive variables as they are now (two a
        // tick in a burst)
        let slices = if self.burst_floats > 0 { 2 } else if self.ticks % KEY_EVERY == 0 { 1 } else { 0 };
        for _ in 0..if n > 0 { slices } else { 0 } {
            let first = self.key_float % n;
            let take = ((ROOM - HEADER - 4) / 4).min(n - first);
            // (only a run of ids one after another can go as a run: they mostly are)
            let mut run = 1;
            while run < take && float_ids[first + run] == float_ids[first] + run as u16 {
                run += 1;
            }
            let mut d = header(protocol, id, table, 1);
            d.extend_from_slice(&float_ids[first].to_le_bytes());
            d.extend_from_slice(&(run as u16).to_le_bytes());
            for k in first..first + run {
                d.extend_from_slice(&floats[k].to_le_bytes());
                self.floats[k] = floats[k];
            }
            self.key_float = (first + run) % n;
            self.burst_floats = self.burst_floats.saturating_sub(run);
            out.push(d);
        }
        // strings: those that changed, and in turn the others
        let m = strings.len().min(string_ids.len());
        if m > 0 {
            let key = self.burst_strings > 0 || self.ticks % STRINGS_KEY_EVERY == 0;
            let mut d = header(protocol, id, table, 2);
            d.extend_from_slice(&0u16.to_le_bytes());
            let mut count = 0u16;
            let put = |d: &mut Vec<u8>, k: usize, count: &mut u16| -> bool {
                let s = cut(&strings[k]);
                if d.len() + 4 + s.len() > ROOM {
                    return false;
                }
                d.extend_from_slice(&string_ids[k].to_le_bytes());
                d.extend_from_slice(&(s.len() as u16).to_le_bytes());
                d.extend_from_slice(s.as_bytes());
                *count += 1;
                true
            };
            // (those that went lately go again first: a lost datagram is mended at once)
            let mut recent = std::mem::take(&mut self.recent);
            recent.retain_mut(|(k, left)| {
                if *k >= m || strings[*k] != self.strings[*k] || !put(&mut d, *k, &mut count) {
                    return *k < m && strings[*k] == self.strings[*k];
                }
                *left -= 1;
                *left > 0
            });
            for k in 0..m {
                if strings[k] != self.strings[k] && put(&mut d, k, &mut count) {
                    self.strings[k] = strings[k].clone();
                    recent.retain(|(r, _)| *r != k);
                    recent.push((k, STRING_REPEATS));
                }
            }
            self.recent = recent;
            if key {
                let mut k = self.key_string % m;
                for _ in 0..m {
                    if !put(&mut d, k, &mut count) {
                        break;
                    }
                    self.strings[k] = strings[k].clone();
                    self.burst_strings = self.burst_strings.saturating_sub(1);
                    k = (k + 1) % m;
                }
                self.key_string = k;
            }
            if count > 0 {
                d[HEADER..HEADER + 2].copy_from_slice(&count.to_le_bytes());
                out.push(d);
            }
        }
        out
    }
}

/// A `VARS` datagram read (None: not one, another protocol, or cut short).
pub fn decode(data: &[u8], protocol: u8) -> Option<VarsIn> {
    if data.len() < HEADER + 2 || data[0] != VARS_MAGIC || data[1] != protocol {
        return None;
    }
    let u16_at = |at: usize| -> Option<u16> { Some(u16::from_le_bytes(data.get(at..at + 2)?.try_into().ok()?)) };
    let f32_at = |at: usize| -> Option<f32> { Some(f32::from_le_bytes(data.get(at..at + 4)?.try_into().ok()?)) };
    let id = u32::from_le_bytes(data[2..6].try_into().ok()?);
    let table = u32::from_le_bytes(data[6..10].try_into().ok()?);
    let mut v = VarsIn { id, table, ..VarsIn::default() };
    let mut at = HEADER;
    match data[10] {
        0 => {
            let count = u16_at(at)? as usize;
            at += 2;
            for _ in 0..count {
                v.floats.push((u16_at(at)?, f32_at(at + 2)?));
                at += 6;
            }
        }
        1 => {
            let first = u16_at(at)?;
            let count = u16_at(at + 2)?;
            at += 4;
            for k in 0..count {
                v.floats.push((first.checked_add(k)?, f32_at(at)?));
                at += 4;
            }
        }
        2 => {
            let count = u16_at(at)? as usize;
            at += 2;
            for _ in 0..count {
                let idx = u16_at(at)?;
                let len = u16_at(at + 2)? as usize;
                let s = std::str::from_utf8(data.get(at + 4..at + 4 + len)?).ok()?;
                v.strings.push((idx, s.to_string()));
                at += 4 + len;
            }
        }
        _ => return None,
    }
    Some(v)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What goes out is what comes in: the changes at once, the rest by key frame slices,
    /// strings too - and after a few seconds every variable has arrived, whatever was lost.
    #[test]
    fn every_variable_arrives_and_changes_go_at_once() {
        let n = 3000;
        let ids: Vec<u16> = (0..n as u16).collect();
        let mut floats: Vec<f32> = (0..n).map(|k| k as f32 * 0.5).collect();
        let sids: Vec<u16> = (0..40).collect();
        let mut strings: Vec<String> = (0..40).map(|k| format!("text {k}")).collect();
        let mut tx = VarSender::default();
        let mut got = vec![f32::NAN; n];
        let mut got_s = vec![String::new(); 40];
        let mut take = |msgs: Vec<Vec<u8>>, got: &mut Vec<f32>, got_s: &mut Vec<String>| {
            for m in msgs {
                assert!(m.len() <= crate::MAX_DATAGRAM, "{}", m.len());
                let v = decode(&m, 5).unwrap();
                assert_eq!((v.id, v.table), (7, 99));
                for (i, x) in v.floats {
                    got[i as usize] = x;
                }
                for (i, s) in v.strings {
                    got_s[i as usize] = s;
                }
            }
        };
        // the first message after a change carries it
        take(tx.tick(5, 7, 99, &ids, &floats, &sids, &strings, 0.1), &mut got, &mut got_s);
        floats[2500] = -1.0;
        strings[3] = "Hauptbahnhof".into();
        take(tx.tick(5, 7, 99, &ids, &floats, &sids, &strings, 0.1), &mut got, &mut got_s);
        // (the first tick sent everything changed from "unknown", as far as a datagram holds)
        for _ in 0..200 {
            take(tx.tick(5, 7, 99, &ids, &floats, &sids, &strings, 0.1), &mut got, &mut got_s);
        }
        assert_eq!(got, floats);
        assert_eq!(got_s, strings);
        // an unchanged state sends little: the key frame slices only (some 6 KB a second)
        let quiet: usize = (0..10).map(|_| tx.tick(5, 7, 99, &ids, &floats, &sids, &strings, 0.1).iter().map(Vec::len).sum::<usize>()).sum();
        assert!(quiet < 6 * ROOM, "{quiet} bytes a second for nothing new");
        // one change: in the very next datagram
        floats[17] = 4.25;
        let m = tx.tick(5, 7, 99, &ids, &floats, &sids, &strings, 0.1);
        let v = decode(&m[0], 5).unwrap();
        assert_eq!(v.floats, vec![(17, 4.25)]);
    }

    /// Everything a datagram of the sender carries, as (floats, strings).
    fn sent(msgs: &[Vec<u8>]) -> (Vec<(u16, f32)>, Vec<(u16, String)>) {
        let (mut f, mut s) = (Vec::new(), Vec::new());
        for m in msgs {
            let v = decode(m, 5).unwrap();
            f.extend(v.floats);
            s.extend(v.strings);
        }
        (f, s)
    }

    #[test]
    fn many_changes_go_in_one_tick_and_a_changed_text_goes_thrice() {
        let n = 1000;
        let ids: Vec<u16> = (0..n as u16).collect();
        let mut floats: Vec<f32> = vec![0.0; n];
        let (sids, mut strings): (Vec<u16>, Vec<String>) = ((0..4).collect(), vec![String::new(); 4]);
        let mut tx = VarSender::default();
        for _ in 0..60 {
            tx.tick(5, 7, 99, &ids, &floats, &sids, &strings, 0.1);
        }
        // 600 values changed at once (a bus on the move): all of them in the next tick
        for x in floats.iter_mut().take(600) {
            *x += 1.0;
        }
        let (f, _) = sent(&tx.tick(5, 7, 99, &ids, &floats, &sids, &strings, 0.1));
        assert!(f.iter().filter(|(i, _)| *i < 600).count() == 600, "{} of 600", f.len());
        // a display's text: in this tick and the next two again, whatever else goes
        strings[2] = "Hbf".into();
        let times = (0..6)
            .filter(|_| sent(&tx.tick(5, 7, 99, &ids, &floats, &sids, &strings, 0.1)).1.iter().any(|(i, s)| *i == 2 && s == "Hbf"))
            .count();
        assert!(times >= 3, "{times}");
    }

    #[test]
    fn a_burst_sends_everything_within_a_few_ticks() {
        let n = 3000;
        let ids: Vec<u16> = (0..n as u16).collect();
        let floats: Vec<f32> = (0..n).map(|k| k as f32).collect();
        let sids: Vec<u16> = (0..40).collect();
        let strings: Vec<String> = (0..40).map(|k| format!("line {k}")).collect();
        let mut tx = VarSender::default();
        for _ in 0..80 {
            tx.tick(5, 7, 99, &ids, &floats, &sids, &strings, 0.1);
        }
        // a player joins: everything again, at once
        tx.burst();
        let (mut got_f, mut got_s) = (std::collections::HashSet::new(), std::collections::HashSet::new());
        for _ in 0..6 {
            let (f, s) = sent(&tx.tick(5, 7, 99, &ids, &floats, &sids, &strings, 0.1));
            got_f.extend(f.into_iter().map(|x| x.0));
            got_s.extend(s.into_iter().map(|x| x.0));
        }
        assert_eq!((got_f.len(), got_s.len()), (n, 40));
    }

    #[test]
    fn a_short_or_foreign_datagram_is_not_taken() {
        let mut tx = VarSender::default();
        let m = tx.tick(5, 1, 2, &[0, 1], &[1.0, 2.0], &[0], &["x".into()], 0.1);
        assert!(decode(&m[0], 4).is_none(), "another protocol");
        assert!(decode(&m[0][..m[0].len() - 1], 5).is_none(), "cut short");
        assert!(decode(b"STATE|1", 5).is_none());
    }
}
