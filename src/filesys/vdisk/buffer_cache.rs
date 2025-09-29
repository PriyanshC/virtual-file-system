use std::collections::{HashMap, VecDeque};

use crate::filesys::block::BlockOperations;
use crate::Size;

use super::block;

struct CacheBlock {
  data: [u8; block::BLOCK_USIZE],
  is_dirty: bool,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BlockLocation {
  // Resident in memory
  T1,
  T2,
  // Not resident (ghost)
  B1,
  B2,
}

pub struct ArcCacheDisk<'a> {
  block_device: Box<dyn block::BlockOperations + 'a>,
  capacity: usize,
  p: usize, // Adaptive target size for T1
  t1: VecDeque<Size>, // Recent list
  t2: VecDeque<Size>, // Frequent list  
  b1: VecDeque<Size>, // Ghost recent list
  b2: VecDeque<Size>, // Ghost frequent list
  
  data_store: HashMap<Size, CacheBlock>,
  page_map: HashMap<Size, BlockLocation>,
}

impl<'a> ArcCacheDisk<'a> {
  pub fn new<D: BlockOperations + 'a>(disk: D, capacity: usize) -> Self {
    assert!(capacity > 0, "Capacity must be greater than 0");
    ArcCacheDisk { 
      block_device: Box::new(disk),
      p: 0,
      capacity: capacity,
      data_store: HashMap::with_capacity(capacity),
      t1: VecDeque::with_capacity(capacity),
      t2: VecDeque::with_capacity(capacity),
      b1: VecDeque::with_capacity(capacity),
      b2: VecDeque::with_capacity(capacity),
      page_map: HashMap::with_capacity(capacity * 2),
    }
  }
  
  fn remove_from_list(&mut self, pos: &Size, from: BlockLocation) {
    let list = match from {
      BlockLocation::T1 => &mut self.t1,
      BlockLocation::T2 => &mut self.t2,
      BlockLocation::B1 => &mut self.b1,
      BlockLocation::B2 => &mut self.b2,
    };
    
    list.iter().position(|&p| p == *pos).map(|i| {
      list.remove(i);
      i
    });
  }
  
  fn add_to_mru(&mut self, pos: &Size, to: BlockLocation) {
    let list = match to {
      BlockLocation::T1 => &mut self.t1,
      BlockLocation::T2 => &mut self.t2,
      loc => unreachable!("Tried to add to MRU of ghost list {:?}", loc),
    };
    list.push_back(*pos);
    self.page_map.insert(*pos, to);
  }
  
  fn move_page(&mut self, pos: &Size, from: BlockLocation, to: BlockLocation) {
    self.remove_from_list(pos, from);
    self.add_to_mru(pos, to);
  }
  
  fn evict(&mut self) {
    
  }
}

impl<'a> block::BlockOperations for ArcCacheDisk<'a> {
  fn read(&mut self, buf: &mut [u8; block::BLOCK_USIZE], pos: crate::Size) {
  }
  
  fn write(&mut self, buf: &[u8; block::BLOCK_USIZE], pos: crate::Size) {
    if let Some(block) = self.data_store.get_mut(&pos) {
      // Cache hit
      block.is_dirty = true;
      block.data.copy_from_slice(buf);
      
      // Promote to MRU of T2
      let location = self.page_map.get(&pos).copied().unwrap();
      self.move_page(&pos, location, BlockLocation::T2);
    } else {
      // Miss

      if self.t1.len() + self.t2.len() >= self.capacity {
        self.evict();
      }
      
      // Restore into T1
      self.add_to_mru(&pos, BlockLocation::T1);
      let new_block = CacheBlock {
        data: *buf,
        is_dirty: true,
      };
      self.data_store.insert(pos, new_block);
    }
  }
  
  fn flush(&mut self) {
    for (pos, block) in self.data_store.iter_mut() {
      if block.is_dirty {
        self.block_device.write(&block.data, *pos);
        block.is_dirty = false;
      }
    }
    self.block_device.flush();
  }
  
  fn stats(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    self.block_device.stats(f)
  }
}
