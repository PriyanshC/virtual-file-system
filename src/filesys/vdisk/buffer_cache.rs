use std::collections::{HashMap, VecDeque};

use crate::filesys::block::BlockOperations;

use super::block;
use super::VDisk;

pub trait BufferCacheDisk : block::BlockOperations {
  fn flush(&mut self);
}

struct CacheBlock {
  data: Box<[u8; block::BLOCK_USIZE]>,
  is_dirty: bool,
}

pub struct ArcCacheDisk {
  disk: VDisk,
  p: usize,
  capacity: usize,
  
  data_store: HashMap<crate::Size, CacheBlock>,
  
  t1: VecDeque<crate::Size>, // Recency cache
  t2: VecDeque<crate::Size>, // Frequency cache
  b1: VecDeque<crate::Size>, // Recency ghost list
  b2: VecDeque<crate::Size>, // Frequency ghost list
}

impl ArcCacheDisk {
  pub fn new(disk: VDisk, capacity: usize) -> Self {
    ArcCacheDisk { 
      disk,
      p: 0,
      capacity: capacity,
      data_store: HashMap::with_capacity(capacity),
      t1: VecDeque::with_capacity(capacity),
      t2: VecDeque::with_capacity(capacity),
      b1: VecDeque::with_capacity(capacity),
      b2: VecDeque::with_capacity(capacity),
    }
  }
  
  /// Called when the cache is full and a new page needs to be loaded
  fn replace(&mut self, is_b1_hit: bool) {
    if self.t1.len() >= 1 && (is_b1_hit || self.t1.len() > self.p) {
      // Evict from T1, moving it to the ghost list B1
      if let Some(pos) = self.t1.pop_back() {
      #[cfg(feature = "debug")]
        println!("[CACHE] Evicting {} from T1 to B1", pos);
        self.b1.push_front(pos);
        self.data_store.remove(&pos);
      }
    } else {
      // Evict from T2, moving it to the ghost list B2.
      if let Some(pos) = self.t2.pop_back() {
        #[cfg(feature = "debug")]
        println!("[CACHE] Evicting {} from T2 to B2", pos);
        self.b2.push_front(pos);
        self.data_store.remove(&pos);
      }
    }
  }
  
  /// This is used for promotion on a cache hit.
  fn move_to_t2(&mut self, list: &mut VecDeque<crate::Size>, pos: crate::Size) {
    if let Some(index) = list.iter().position(|&p| p == pos) {
      list.remove(index);
    }
    self.t2.push_front(pos);
  }
}

impl block::BlockOperations for ArcCacheDisk {
  fn read(&mut self, buf: &mut [u8; block::BLOCK_USIZE], pos: crate::Size) {
    todo!()
  }
  
  fn write(&mut self, buf: &[u8; block::BLOCK_USIZE], pos: crate::Size) {
    let mut temp_buf = [0u8; block::BLOCK_USIZE];
    self.read(&mut temp_buf, pos);
    
    #[cfg(feature = "debug")]
    println!("[CACHE] Writing data to block {}", pos);
    
    let block = self.data_store.get_mut(&pos).expect("Block must be in cache after read");
    block.data.copy_from_slice(buf);
    block.is_dirty = true;
  }
}

impl BufferCacheDisk for ArcCacheDisk {
  fn flush(&mut self) {
    #[cfg(feature = "debug")]
    println!("[CACHE] Flushing all dirty blocks...");
    
    for (pos, block) in self.data_store.iter_mut() {
      if block.is_dirty {
        self.disk.write(&block.data, *pos);
        block.is_dirty = false;
      }
    }
    
    #[cfg(feature = "debug")]
    println!("[CACHE] Flush complete.");
  }
}