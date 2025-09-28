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
}

impl block::BlockOperations for ArcCacheDisk {
  fn read(&mut self, buf: &mut [u8; block::BLOCK_USIZE], pos: crate::Size) {
    todo!()
  }
  
  fn write(&mut self, buf: &[u8; block::BLOCK_USIZE], pos: crate::Size) {
    todo!()
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