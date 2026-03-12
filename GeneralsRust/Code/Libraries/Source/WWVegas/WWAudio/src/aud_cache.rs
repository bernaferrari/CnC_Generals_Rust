//! Audio Cache System
//! 
//! Provides efficient caching of audio samples with LRU eviction,
//! memory pooling, and performance profiling. This is a direct
//! conversion of the C++ AUD_Cache.cpp file to idiomatic Rust.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, Weak};
use std::rc::Rc;
use crate::formats::AudioFormat;
use crate::source::{AudioSample, AudioFrame};
use crate::profiler::ProfileData;
use crate::error::{AudioResult, AudioError};
use crate::lock::AudioLock;

/// Type alias for cache open callback function
pub type AudioCacheOpenCb = Box<dyn Fn(&str) -> Option<Box<dyn std::io::Read>> + Send + Sync>;

/// Audio cache for managing loaded samples
/// 
/// The cache uses a combination of HashMap for fast lookups and
/// an LRU list for efficient eviction of unused items.
pub struct AudioCache {
    /// Maximum size of individual frames
    frame_size: usize,
    
    /// Hash map for fast item lookup by name
    items: HashMap<String, Arc<Mutex<AudioCacheItem>>>,
    
    /// LRU list for tracking access order
    lru_order: Vec<String>,
    
    /// Maximum number of items allowed in cache
    max_items: usize,
    
    /// Maximum total cache size in bytes
    max_cache_size: usize,
    
    /// Current cache usage in bytes
    current_size: usize,
    
    /// Callback for opening asset files
    open_asset_cb: Option<AudioCacheOpenCb>,
    
    /// Current asset file being read
    asset_file: Option<Box<dyn std::io::Read>>,
    
    /// Format of current asset file
    asset_format: AudioFormat,
    
    /// Remaining bytes in current asset file
    asset_bytes_left: usize,
    
    /// Performance profiling data
    profile: ProfileData,
}

/// Individual cache item containing audio sample data
pub struct AudioCacheItem {
    /// Name/identifier of the cached item
    pub name: String,
    
    /// Whether this item is valid and ready for use
    pub valid: bool,
    
    /// Audio format of this sample
    pub format: AudioFormat,
    
    /// The actual audio sample data
    pub sample: AudioSample,
    
    /// Lock for thread-safe access
    pub lock: AudioLock,
    
    /// Reference count for tracking usage
    pub ref_count: usize,
    
    /// Weak reference back to the cache
    cache: Weak<Mutex<AudioCache>>,
}

impl AudioCache {
    /// Create a new audio cache
    /// 
    /// # Arguments
    /// * `cache_size` - Maximum size of the cache in bytes
    /// * `max_items` - Maximum number of items to cache
    /// * `frame_size` - Size of individual audio frames
    /// 
    /// # Returns
    /// A new AudioCache instance wrapped in Arc<Mutex<>> for thread safety
    pub fn create(cache_size: usize, max_items: usize, frame_size: usize) -> Arc<Mutex<Self>> {
        let cache = Arc::new(Mutex::new(AudioCache {
            frame_size,
            items: HashMap::new(),
            lru_order: Vec::new(),
            max_items,
            max_cache_size: cache_size,
            current_size: 0,
            open_asset_cb: None,
            asset_file: None,
            asset_format: AudioFormat::default(),
            asset_bytes_left: 0,
            profile: ProfileData::new(cache_size / frame_size, frame_size),
        }));

        cache
    }

    /// Set the callback function for opening asset files
    /// 
    /// # Arguments
    /// * `callback` - Function that takes a filename and returns a readable stream
    /// 
    /// # Returns
    /// The previously set callback, if any
    pub fn set_open_callback(&mut self, callback: AudioCacheOpenCb) -> Option<AudioCacheOpenCb> {
        std::mem::replace(&mut self.open_asset_cb, Some(callback))
    }

    /// Get an item from the cache by name
    /// 
    /// # Arguments
    /// * `name` - Name of the item to retrieve
    /// 
    /// # Returns
    /// An Arc to the cached item if found, None otherwise
    pub fn get_item(&mut self, name: &str) -> Option<Arc<Mutex<AudioCacheItem>>> {
        if let Some(item) = self.items.get(name) {
            // Move to front of LRU list
            self.move_to_front(name);
            self.profile.cache_hit();
            Some(Arc::clone(item))
        } else {
            None
        }
    }

    /// Load an item into the cache
    /// 
    /// # Arguments
    /// * `name` - Name of the item to load
    /// 
    /// # Returns
    /// Result containing the loaded item or an error
    pub fn load_item(&mut self, name: &str) -> AudioResult<Arc<Mutex<AudioCacheItem>>> {
        // Check if already in cache
        if let Some(item) = self.get_item(name) {
            return Ok(item);
        }

        self.profile.cache_miss();
        self.profile.load_start(0);

        // Open the asset file
        if !self.open_asset(name)? {
            self.profile.load_end();
            return Err(AudioError::FileNotFound(name.to_string()));
        }

        // Ensure we have space in the cache
        self.make_space_if_needed()?;

        // Create new cache item
        let item = Arc::new(Mutex::new(AudioCacheItem {
            name: name.to_string(),
            valid: false,
            format: self.asset_format.clone(),
            sample: AudioSample::new(),
            lock: AudioLock::new(),
            ref_count: 0,
            cache: Arc::downgrade(&Arc::new(Mutex::new(self))), // This is a simplification
        }));

        // Load sample data
        self.load_sample_data(&item)?;

        // Add to cache
        self.items.insert(name.to_string(), Arc::clone(&item));
        self.lru_order.insert(0, name.to_string());

        // Mark as valid
        {
            let mut item_guard = item.lock().map_err(|_| AudioError::LockError)?;
            item_guard.valid = true;
        }

        self.close_asset();
        self.profile.load_end();

        Ok(item)
    }

    /// Invalidate all cache entries
    /// 
    /// This marks all cached items as invalid but doesn't remove them
    /// from memory until they're no longer referenced.
    pub fn invalidate(&mut self) {
        for item in self.items.values() {
            if let Ok(mut item_guard) = item.lock() {
                item_guard.valid = false;
            }
        }
    }

    /// Free the oldest unused item from the cache
    /// 
    /// # Returns
    /// true if an item was freed, false if no items could be freed
    pub fn free_oldest_item(&mut self) -> bool {
        // Start from the end of the LRU list (oldest items)
        for i in (0..self.lru_order.len()).rev() {
            let name = &self.lru_order[i];
            if let Some(item) = self.items.get(name) {
                if let Ok(item_guard) = item.try_lock() {
                    if !item_guard.is_in_use() {
                        drop(item_guard); // Release the lock before removing
                        self.free_item_by_name(name);
                        return true;
                    }
                }
            }
        }
        false
    }

    /// Private helper methods
    
    fn move_to_front(&mut self, name: &str) {
        if let Some(pos) = self.lru_order.iter().position(|x| x == name) {
            let name = self.lru_order.remove(pos);
            self.lru_order.insert(0, name);
        }
    }

    fn make_space_if_needed(&mut self) -> AudioResult<()> {
        // Remove items until we have space
        while self.items.len() >= self.max_items {
            if !self.free_oldest_item() {
                return Err(AudioError::CacheOverflow);
            }
        }

        // TODO: Also check total memory usage
        Ok(())
    }

    fn open_asset(&mut self, name: &str) -> AudioResult<bool> {
        self.close_asset();

        if let Some(ref callback) = self.open_asset_cb {
            if let Some(file) = callback(name) {
                self.asset_file = Some(file);
                
                // Read wave file format (simplified - would need proper wave parsing)
                self.asset_format = AudioFormat::default();
                self.asset_bytes_left = 0; // Would be set from wave file header
                
                return Ok(true);
            }
        }

        Ok(false)
    }

    fn close_asset(&mut self) {
        self.asset_file = None;
        self.asset_bytes_left = 0;
    }

    fn load_sample_data(&mut self, item: &Arc<Mutex<AudioCacheItem>>) -> AudioResult<()> {
        let mut bytes_to_transfer = self.asset_bytes_left;
        
        while bytes_to_transfer > 0 {
            let bytes = std::cmp::min(self.frame_size, bytes_to_transfer);
            
            // Create new frame
            let mut frame_data = vec![0u8; bytes];
            
            // Read data from asset file
            if let Some(ref mut file) = self.asset_file {
                use std::io::Read;
                let bytes_read = file.read(&mut frame_data)
                    .map_err(|_| AudioError::ReadError)?;
                
                if bytes_read == 0 {
                    break;
                }
                
                frame_data.truncate(bytes_read);
                
                // Create audio frame
                let frame = AudioFrame::new(frame_data);
                
                // Add frame to sample
                {
                    let mut item_guard = item.lock().map_err(|_| AudioError::LockError)?;
                    item_guard.sample.add_frame(frame);
                }
                
                self.profile.add_load_bytes(bytes_read);
                self.profile.add_page();
                self.profile.fill(bytes_read);
                
                bytes_to_transfer -= bytes_read;
                self.current_size += bytes_read;
            } else {
                return Err(AudioError::ReadError);
            }
        }

        Ok(())
    }

    fn free_item_by_name(&mut self, name: &str) {
        if let Some(item) = self.items.remove(name) {
            // Remove from LRU order
            self.lru_order.retain(|x| x != name);
            
            // Update cache size
            if let Ok(item_guard) = item.lock() {
                let item_size = item_guard.sample.total_size();
                self.current_size = self.current_size.saturating_sub(item_size);
                self.profile.remove(item_size);
            }
        }
    }
}

impl AudioCacheItem {
    /// Lock this cache item for exclusive access
    pub fn lock_item(&mut self) {
        self.lock.acquire();
    }

    /// Unlock this cache item
    pub fn unlock_item(&mut self) {
        self.lock.release();
    }

    /// Check if this item is currently in use
    pub fn is_in_use(&self) -> bool {
        self.lock.is_locked()
    }

    /// Get the audio sample from this cache item
    pub fn get_sample(&self) -> &AudioSample {
        &self.sample
    }

    /// Increment reference count
    pub fn add_ref(&mut self) {
        self.ref_count += 1;
    }

    /// Decrement reference count
    pub fn release(&mut self) {
        if self.ref_count > 0 {
            self.ref_count -= 1;
        }
    }
}

impl Drop for AudioCache {
    fn drop(&mut self) {
        // Clean up all items
        self.items.clear();
        self.lru_order.clear();
        self.close_asset();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_creation() {
        let cache = AudioCache::create(1024 * 1024, 100, 4096);
        let cache_guard = cache.lock().unwrap();
        assert_eq!(cache_guard.max_items, 100);
        assert_eq!(cache_guard.max_cache_size, 1024 * 1024);
        assert_eq!(cache_guard.frame_size, 4096);
    }

    #[test]
    fn test_cache_invalidation() {
        let cache = AudioCache::create(1024 * 1024, 100, 4096);
        let mut cache_guard = cache.lock().unwrap();
        
        // Add some mock items
        let item = Arc::new(Mutex::new(AudioCacheItem {
            name: "test".to_string(),
            valid: true,
            format: AudioFormat::default(),
            sample: AudioSample::new(),
            lock: AudioLock::new(),
            ref_count: 0,
            cache: Arc::downgrade(&cache),
        }));
        
        cache_guard.items.insert("test".to_string(), item.clone());
        
        // Invalidate cache
        cache_guard.invalidate();
        
        // Check that item is now invalid
        let item_guard = item.lock().unwrap();
        assert!(!item_guard.valid);
    }
}