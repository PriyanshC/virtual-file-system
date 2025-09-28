use std::collections::{HashMap, VecDeque};

use crate::filesys::block::BlockOperations;

use super::block;

struct CacheBlock {
  data: Box<[u8; block::BLOCK_USIZE]>,
  is_dirty: bool,
}

pub struct ArcCacheDisk<'a> {
  block_device: Box<dyn block::BlockOperations + 'a>,
  p: usize,
  capacity: usize,
  
  data_store: HashMap<crate::Size, CacheBlock>,
  
  t1: VecDeque<crate::Size>, // Recency cache
  t2: VecDeque<crate::Size>, // Frequency cache
  b1: VecDeque<crate::Size>, // Recency ghost list
  b2: VecDeque<crate::Size>, // Frequency ghost list
}

impl<'a> ArcCacheDisk<'a> {
  pub fn new<D: BlockOperations + 'a>(disk: D, capacity: usize) -> Self {
    ArcCacheDisk { 
      block_device: Box::new(disk),
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
  fn promote_to_t2(&mut self, pos: crate::Size) {
    // Remove from T1 if present
    if let Some(index) = self.t1.iter().position(|&p| p == pos) {
      self.t1.remove(index);
      self.t2.push_front(pos);
      return;
    }
    
    // If already in T2, move to front
    if let Some(index) = self.t2.iter().position(|&p| p == pos) {
      self.t2.remove(index);
      self.t2.push_front(pos);
    }
  }
}

impl<'a> block::BlockOperations for ArcCacheDisk<'a> {
  fn read(&mut self, buf: &mut [u8; block::BLOCK_USIZE], pos: crate::Size) {
    // Case 1: Hit in T1 or T2 (the page is in the cache)
    if self.t1.contains(&pos) || self.t2.contains(&pos) {
      #[cfg(feature = "debug")]
      println!("[CACHE] Hit for block {}", pos);
      self.promote_to_t2(pos);
      let block = self.data_store.get(&pos).expect("Data should exist on hit");
      buf.copy_from_slice(&*block.data);
      return;
    }
    
    #[cfg(feature = "debug")]
    println!("[CACHE] Miss for block {}", pos);
    // Case 2: Miss, but the page is in a ghost list
    if self.b1.contains(&pos) {
      // Adapt p: Increase target size for T1
      self.p = (self.p + 1).min(self.capacity);
      self.replace(true);
    } else if self.b2.contains(&pos) {
      // Adapt p: Decrease target size for T1
      self.p = self.p.saturating_sub(1);
      self.replace(false);
    } else {
      // Case 3: Cold miss (page seen for the first time)
      if self.t1.len() + self.t2.len() >= self.capacity {
        self.replace(false);
      }
    }
    
    let mut data = [0u8; block::BLOCK_USIZE];
    self.block_device.read(&mut data, pos);
    let new_block = CacheBlock { data: Box::new(data), is_dirty: false };
    self.data_store.insert(pos, new_block);
    
    self.t1.push_front(pos);
    
    let block = self.data_store.get(&pos).unwrap();
    buf.copy_from_slice(&*block.data);
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

  fn flush(&mut self) {
    #[cfg(feature = "debug")]
    println!("[CACHE] Flushing all dirty blocks...");
    
    for (pos, block) in self.data_store.iter_mut() {
      if block.is_dirty {
        self.block_device.write(&block.data, *pos);
        block.is_dirty = false;
      }
    }
    
    #[cfg(feature = "debug")]
    println!("[CACHE] Flush complete.");
  }
}
