//! Textures the CPU compressed, kept on disk between runs (`~/.openomsi/cache/textures`).
//!
//! Most of OMSI 2's textures are not DDS files (BMP, TGA, JPG): each was compressed to BC1
//! or BC3 with its whole mip chain every time a map loaded - 16 of the 17 s the first area
//! of the Thüringer Wald took. The second time, the blocks are read back from here.
//!
//! An entry is found by the texture's path, the size and time of the file it is read from
//! (or of the archive it is in) and the GPU options; a changed file is compressed again.

use crate::gpu::{GpuOptions, PixelFormat};
use crate::TextureData;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Bump when what the loaders make of a picture changes (the encoder, its mip filter, the
/// quality it must keep): entries of another version are not read.
pub const FORMAT_VERSION: u32 = 1;
/// What the cache may take on the disk; the entries used longest ago go first.
pub const CAP_BYTES: u64 = 4 << 30;
const MAGIC: &[u8; 4] = b"OTXC";
/// A half-written entry older than this was left by a run that stopped.
const STALE_TEMP: Duration = Duration::from_secs(600);

/// What an entry says about a texture.
#[derive(Debug, Clone)]
pub enum Hit {
    /// The texture as it goes to the GPU.
    Data(TextureData),
    /// Compressing it was given up for quality: it goes up as the RGBA it decodes to.
    Plain,
}

/// The folder of the entries.
pub struct DiskCache {
    dir: PathBuf,
}

impl DiskCache {
    pub fn new(dir: PathBuf) -> DiskCache {
        DiskCache { dir }
    }

    /// The entry of `path` under the options `o`, if one was kept for the file as it is now.
    pub fn get(&self, path: &Path, o: GpuOptions) -> Option<Hit> {
        let tag = tag(path, o)?;
        let file = self.file(&tag);
        let Ok(bytes) = std::fs::read(&file) else {
            MISSES.fetch_add(1, Ordering::Relaxed);
            return None;
        };
        let hit = parse(&bytes, &tag);
        match hit {
            Some(_) => {
                HITS.fetch_add(1, Ordering::Relaxed);
                // (used now: `prune` lets the ones used longest ago go)
                if let Ok(f) = std::fs::File::options().write(true).open(&file) {
                    let _ = f.set_modified(SystemTime::now());
                }
            }
            None => {
                let _ = std::fs::remove_file(&file);
            }
        }
        hit
    }

    /// Keep what `path` became under the options `o`.
    pub fn put(&self, path: &Path, o: GpuOptions, hit: &Hit) {
        match hit {
            Hit::Data(t) => self.put_data(path, o, t),
            Hit::Plain => self.write(path, o, None),
        }
    }

    /// Keep the texture `path` became under the options `o`.
    pub fn put_data(&self, path: &Path, o: GpuOptions, t: &TextureData) {
        self.write(path, o, Some(t));
    }

    fn write(&self, path: &Path, o: GpuOptions, t: Option<&TextureData>) {
        let Some(tag) = tag(path, o) else { return };
        let file = self.file(&tag);
        let bytes = entry(&tag, t);
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let temp = file.with_extension(format!("{}-{}.tmp", std::process::id(), NEXT.fetch_add(1, Ordering::Relaxed)));
        // (written aside and renamed: a run that stops half way leaves no half entry)
        let done = file
            .parent()
            .map_or(Ok(()), std::fs::create_dir_all)
            .and_then(|_| std::fs::write(&temp, &bytes))
            .and_then(|_| std::fs::rename(&temp, &file));
        if let Err(e) = done {
            let _ = std::fs::remove_file(&temp);
            static TOLD: AtomicBool = AtomicBool::new(false);
            if !TOLD.swap(true, Ordering::Relaxed) {
                log::warn!("texture cache: could not keep {} in {}: {e}", path.display(), self.dir.display());
            }
        }
    }

    fn file(&self, tag: &str) -> PathBuf {
        let key = fnv1a(tag.as_bytes());
        self.dir.join(format!("{:02x}", key >> 56)).join(format!("{key:016x}.otc"))
    }

    /// Delete the entries used longest ago until they take no more than `cap` bytes.
    /// Returns (entries deleted, bytes kept).
    pub fn prune(&self, cap: u64) -> (usize, u64) {
        let now = SystemTime::now();
        let mut entries: Vec<(SystemTime, u64, PathBuf)> = Vec::new();
        let mut gone = 0;
        for sub in std::fs::read_dir(&self.dir).into_iter().flatten().flatten() {
            for e in std::fs::read_dir(sub.path()).into_iter().flatten().flatten() {
                let Ok(m) = e.metadata() else { continue };
                if !m.is_file() {
                    continue;
                }
                let at = m.modified().unwrap_or(UNIX_EPOCH);
                let p = e.path();
                if p.extension().is_some_and(|x| x == "tmp") {
                    if now.duration_since(at).unwrap_or_default() > STALE_TEMP && std::fs::remove_file(&p).is_ok() {
                        gone += 1;
                    }
                    continue;
                }
                entries.push((at, m.len(), p));
            }
        }
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        let mut kept: u64 = entries.iter().map(|e| e.1).sum();
        for (_, len, p) in &entries {
            if kept <= cap {
                break;
            }
            if std::fs::remove_file(p).is_ok() {
                kept -= len;
                gone += 1;
            }
        }
        (gone, kept)
    }
}

/// Where the picture of `path` stands now under the options `o`: its file's (or its
/// archive's) size and time. None when neither can be looked at.
fn tag(path: &Path, o: GpuOptions) -> Option<String> {
    // (as `vfs::read` reads it: from a mounted archive before the folder)
    let (from, m) = match omsi_cfg::vfs::archive_of(path) {
        Some(a) => (format!("zip {}", a.display()), std::fs::metadata(&a).ok()?),
        None => (String::from("file"), std::fs::metadata(path).ok()?),
    };
    let at = m.modified().ok()?.duration_since(UNIX_EPOCH).ok()?.as_nanos();
    let opts = o.bc as u8 | (o.compress as u8) << 1;
    Some(format!("{}|{from}|{}|{at}|{opts}", path.display(), m.len()))
}

/// FNV-1a: the file name of an entry (stable between builds, unlike std's hasher).
fn fnv1a(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |h, &b| (h ^ b as u64).wrapping_mul(0x0000_0100_0000_01b3))
}

fn format_code(f: PixelFormat) -> u8 {
    match f {
        PixelFormat::Rgba8 => 0,
        PixelFormat::Bc1 => 1,
        PixelFormat::Bc2 => 2,
        PixelFormat::Bc3 => 3,
    }
}

fn entry(tag: &str, t: Option<&TextureData>) -> Vec<u8> {
    let size = t.map_or(0, |t| t.cpu_bytes() + 4 * t.levels.len());
    let mut b = Vec::with_capacity(64 + tag.len() + size);
    b.extend_from_slice(MAGIC);
    b.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    b.extend_from_slice(&(tag.len() as u32).to_le_bytes());
    b.extend_from_slice(tag.as_bytes());
    match t {
        None => b.push(1),
        Some(t) => {
            b.push(0);
            b.extend_from_slice(&t.width.to_le_bytes());
            b.extend_from_slice(&t.height.to_le_bytes());
            b.extend_from_slice(&[format_code(t.format), t.has_alpha as u8, t.gpu_mips as u8]);
            b.extend_from_slice(&(t.levels.len() as u32).to_le_bytes());
            for l in &t.levels {
                b.extend_from_slice(&(l.len() as u32).to_le_bytes());
                b.extend_from_slice(l);
            }
        }
    }
    b
}

/// An entry read back; None unless it is whole, of this version and for `tag`.
fn parse(b: &[u8], tag: &str) -> Option<Hit> {
    let mut at = 0usize;
    let mut take = |n: usize| -> Option<&[u8]> {
        let s = b.get(at..at.checked_add(n)?)?;
        at += n;
        Some(s)
    };
    let u32_of = |s: &[u8]| u32::from_le_bytes([s[0], s[1], s[2], s[3]]);
    if take(4)? != MAGIC || u32_of(take(4)?) != FORMAT_VERSION {
        return None;
    }
    let n = u32_of(take(4)?) as usize;
    if take(n)? != tag.as_bytes() {
        return None;
    }
    let hit = match take(1)?[0] {
        1 => Hit::Plain,
        0 => {
            let width = u32_of(take(4)?);
            let height = u32_of(take(4)?);
            let f = take(3)?;
            let format = match f[0] {
                0 => PixelFormat::Rgba8,
                1 => PixelFormat::Bc1,
                2 => PixelFormat::Bc2,
                3 => PixelFormat::Bc3,
                _ => return None,
            };
            let (has_alpha, gpu_mips) = (f[1] != 0, f[2] != 0);
            let count = u32_of(take(4)?) as usize;
            if count == 0 || count > 32 {
                return None;
            }
            let mut levels = Vec::with_capacity(count);
            for l in 0..count as u32 {
                let len = u32_of(take(4)?) as usize;
                if len != format.level_bytes((width >> l).max(1), (height >> l).max(1)) {
                    return None;
                }
                levels.push(take(len)?.to_vec());
            }
            Hit::Data(TextureData { width, height, format, levels, has_alpha, gpu_mips })
        }
        _ => return None,
    };
    // (nothing after it: a longer file is not this entry)
    (at == b.len()).then_some(hit)
}

static GLOBAL: RwLock<Option<Arc<DiskCache>>> = RwLock::new(None);
static HITS: AtomicU64 = AtomicU64::new(0);
static MISSES: AtomicU64 = AtomicU64::new(0);

/// Textures found in a cache and not found there, since the start.
pub fn counts() -> (u64, u64) {
    (HITS.load(Ordering::Relaxed), MISSES.load(Ordering::Relaxed))
}

/// The cache the loaders use, if the game set one.
pub fn global() -> Option<Arc<DiskCache>> {
    GLOBAL.read().ok().and_then(|g| g.clone())
}

/// Set (or with None, drop) the cache the loaders use, and cut it down to [`CAP_BYTES`]
/// on a thread of its own. `OMSI_NO_TEX_CACHE` keeps it off.
pub fn set_dir(dir: Option<PathBuf>) {
    let cache = dir.filter(|_| omsi_cfg::env::var_os("OMSI_NO_TEX_CACHE").is_none()).map(|d| Arc::new(DiskCache::new(d)));
    if let Some(c) = cache.clone() {
        let _ = std::thread::Builder::new().name("texture cache".into()).spawn(move || {
            let (gone, kept) = c.prune(CAP_BYTES);
            log::info!("texture cache: {} MB in {}{}", kept >> 20, c.dir.display(), if gone > 0 { format!(" ({gone} old entries deleted)") } else { String::new() });
        });
    }
    if let Ok(mut g) = GLOBAL.write() {
        *g = cache;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("openomsi-tex-cache-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn source(dir: &Path, bytes: &[u8]) -> PathBuf {
        let p = dir.join("wall.bmp");
        std::fs::write(&p, bytes).unwrap();
        p
    }

    fn blocks() -> TextureData {
        TextureData {
            width: 8,
            height: 8,
            format: PixelFormat::Bc1,
            levels: vec![(0..32).collect(), (100..108).collect(), (200..208).collect(), vec![7; 8]],
            has_alpha: false,
            gpu_mips: false,
        }
    }

    const ON: GpuOptions = GpuOptions { bc: true, compress: true };

    #[test]
    fn a_compressed_texture_comes_back_from_the_disk() {
        let dir = temp("back");
        let src = source(&dir, b"picture");
        let cache = DiskCache::new(dir.join("cache"));
        cache.put(&src, ON, &Hit::Data(blocks()));
        let Some(Hit::Data(t)) = cache.get(&src, ON) else { panic!("no entry") };
        let want = blocks();
        assert_eq!((t.width, t.height, t.format, t.has_alpha, t.gpu_mips), (want.width, want.height, want.format, want.has_alpha, want.gpu_mips));
        assert_eq!(t.levels, want.levels);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_texture_given_up_for_quality_comes_back_as_plain() {
        let dir = temp("plain");
        let src = source(&dir, b"noise");
        let cache = DiskCache::new(dir.join("cache"));
        cache.put(&src, ON, &Hit::Plain);
        assert!(matches!(cache.get(&src, ON), Some(Hit::Plain)));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_changed_file_is_not_taken_from_the_disk() {
        let dir = temp("changed");
        let src = source(&dir, b"picture");
        let cache = DiskCache::new(dir.join("cache"));
        cache.put(&src, ON, &Hit::Data(blocks()));
        std::fs::write(&src, b"a repainted picture").unwrap();
        assert!(cache.get(&src, ON).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn other_gpu_options_have_their_own_entry() {
        let dir = temp("options");
        let src = source(&dir, b"picture");
        let cache = DiskCache::new(dir.join("cache"));
        cache.put(&src, ON, &Hit::Data(blocks()));
        assert!(cache.get(&src, GpuOptions { bc: true, compress: false }).is_none());
        assert!(cache.get(&src, ON).is_some());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_damaged_entry_is_not_used() {
        let dir = temp("damaged");
        let src = source(&dir, b"picture");
        let cache = DiskCache::new(dir.join("cache"));
        cache.put(&src, ON, &Hit::Data(blocks()));
        let entries: Vec<PathBuf> = walk(&dir.join("cache"));
        assert_eq!(entries.len(), 1, "one entry written");
        // cut off half way (a crash while it was written, a full disk)
        let bytes = std::fs::read(&entries[0]).unwrap();
        std::fs::write(&entries[0], &bytes[..bytes.len() / 2]).unwrap();
        assert!(cache.get(&src, ON).is_none());
        // and nothing at all where the file was
        std::fs::write(&entries[0], b"").unwrap();
        assert!(cache.get(&src, ON).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_file_without_an_entry_has_none() {
        let dir = temp("none");
        let src = source(&dir, b"picture");
        let cache = DiskCache::new(dir.join("cache"));
        assert!(cache.get(&src, ON).is_none());
        // nor does one that is not there at all
        assert!(cache.get(&dir.join("missing.bmp"), ON).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_cache_is_cut_down_to_its_size_oldest_first() {
        let dir = temp("prune");
        let cache = DiskCache::new(dir.join("cache"));
        let mut sources = Vec::new();
        for i in 0..4 {
            let p = dir.join(format!("t{i}.bmp"));
            std::fs::write(&p, format!("picture {i}")).unwrap();
            cache.put(&p, ON, &Hit::Data(blocks()));
            sources.push(p);
        }
        let entries = walk(&dir.join("cache"));
        assert_eq!(entries.len(), 4);
        let sizes: Vec<u64> = entries.iter().map(|e| std::fs::metadata(e).unwrap().len()).collect();
        // t0 used last: it stays, t1 (used longest ago) goes first
        let now = std::time::SystemTime::now();
        for (i, p) in sources.iter().enumerate() {
            let e = entry_of(&cache, p);
            let age = match i {
                0 => 0,
                1 => 400,
                2 => 300,
                _ => 200,
            };
            let f = std::fs::File::options().write(true).open(&e).unwrap();
            f.set_modified(now - std::time::Duration::from_secs(age)).unwrap();
        }
        let two = sizes.iter().max().unwrap() * 2;
        let (gone, kept) = cache.prune(two);
        assert_eq!(gone, 2);
        assert!(kept <= two);
        assert!(cache.get(&sources[0], ON).is_some());
        assert!(cache.get(&sources[3], ON).is_some());
        assert!(cache.get(&sources[1], ON).is_none());
        assert!(cache.get(&sources[2], ON).is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The entry file `src` has in `cache` (found by putting it in a second cache).
    fn entry_of(cache: &DiskCache, src: &Path) -> PathBuf {
        let before = walk(&cache.dir);
        let probe = cache.dir.with_extension("probe");
        let _ = std::fs::remove_dir_all(&probe);
        let other = DiskCache::new(probe.clone());
        other.put(src, ON, &Hit::Plain);
        let name = walk(&probe).first().and_then(|p| p.file_name()).map(|n| n.to_owned()).expect("an entry in the probe");
        let _ = std::fs::remove_dir_all(&probe);
        before.into_iter().find(|p| p.file_name() == Some(name.as_os_str())).expect("entry of the source")
    }

    fn walk(dir: &Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        let Ok(rd) = std::fs::read_dir(dir) else { return out };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                out.extend(walk(&p));
            } else {
                out.push(p);
            }
        }
        out.sort();
        out
    }
}
