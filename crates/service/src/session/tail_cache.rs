use std::collections::HashMap;
use std::path::Path;

const MAX_CACHE_ENTRIES: usize = 5000;

struct CacheEntry<T> {
    signature: String,
    value: T,
}

/// Simple LRU cache keyed by file path with size+mtime signature.
pub struct SessionCache<T> {
    entries: HashMap<String, CacheEntry<T>>,
    order: Vec<String>,
}

impl<T> SessionCache<T> {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            order: Vec::with_capacity(MAX_CACHE_ENTRIES),
        }
    }

    fn make_key(path: &Path) -> Option<String> {
        let canonical = std::fs::canonicalize(path).ok()?;
        Some(canonical.to_string_lossy().to_string())
    }

    fn make_signature(path: &Path) -> Option<String> {
        let stat = std::fs::metadata(path).ok()?;
        let key = Self::make_key(path)?;
        Some(format!(
            "{}::{}:{}",
            key,
            stat.len(),
            stat.modified()
                .ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis())
                .unwrap_or(0)
        ))
    }

    pub fn get(&mut self, path: &Path) -> Option<&T> {
        let key = Self::make_key(path)?;
        let sig = Self::make_signature(path)?;
        let entry = self.entries.get(&key)?;

        if entry.signature != sig {
            return None;
        }

        // Move to end (LRU refresh).
        if let Some(pos) = self.order.iter().position(|k| k == &key) {
            self.order.remove(pos);
            self.order.push(key.clone());
        }

        Some(&entry.value)
    }

    pub fn insert(&mut self, path: &Path, value: T) {
        let key = match Self::make_key(path) {
            Some(k) => k,
            None => return,
        };
        let sig = match Self::make_signature(path) {
            Some(s) => s,
            None => return,
        };

        // Evict oldest if at capacity.
        if !self.entries.contains_key(&key) && self.entries.len() >= MAX_CACHE_ENTRIES {
            if let Some(oldest) = self.order.first().cloned() {
                self.entries.remove(&oldest);
                self.order.remove(0);
            }
        }

        // Remove old position for key refresh.
        if let Some(pos) = self.order.iter().position(|k| k == &key) {
            self.order.remove(pos);
        }

        self.order.push(key.clone());
        self.entries.insert(
            key,
            CacheEntry {
                signature: sig,
                value,
            },
        );
    }
}

impl<T> Default for SessionCache<T> {
    fn default() -> Self {
        Self::new()
    }
}

/// Thread-local session metadata cache.
use std::cell::RefCell;

thread_local! {
    pub static SESSION_META_CACHE: RefCell<SessionCache<crate::session::jsonl_parser::SessionMeta>> =
        RefCell::new(SessionCache::new());
    pub static SESSION_TAIL_CACHE: RefCell<SessionCache<crate::session::jsonl_parser::TailInsights>> =
        RefCell::new(SessionCache::new());
}
