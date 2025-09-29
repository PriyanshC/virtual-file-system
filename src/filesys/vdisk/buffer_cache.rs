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
    debug_assert!(!(self.t1.len() == 0 && self.t2.len() == 0));
    
    let evict_from_t1 = self.t1.len() > 0 && (self.t1.len() > self.p || (self.t2.is_empty()));
    
    let (evicted_pos, ghost_list_loc) = if evict_from_t1 {
      (self.t1.pop_front().unwrap(), BlockLocation::B1)
    } else {
      (self.t2.pop_front().unwrap(), BlockLocation::B2)
    };
    
    if let Some(block) = self.data_store.get(&evicted_pos) {
      if block.is_dirty {
        self.block_device.write(&block.data, evicted_pos);
      }
    }
    
    self.data_store.remove(&evicted_pos);
    self.page_map.insert(evicted_pos, ghost_list_loc);
    match ghost_list_loc {
      BlockLocation::B1 => self.b1.push_back(evicted_pos),
      BlockLocation::B2 => self.b2.push_back(evicted_pos),
      _ => unreachable!(),
    }
  }
}

impl<'a> block::BlockOperations for ArcCacheDisk<'a> {
  fn read(&mut self, buf: &mut [u8; block::BLOCK_USIZE], pos: crate::Size) {
    let location = self.page_map.get(&pos).copied();
    if let Some(loc) = location {
      // Cache hit
      if loc == BlockLocation::T1 || loc == BlockLocation::T2 {
        self.move_page(&pos, loc, BlockLocation::T2);
        buf.copy_from_slice(&self.data_store[&pos].data);
        return;
      }
    };
    
    
    match location {
      Some(BlockLocation::B1) => {
        self.remove_from_list(&pos, BlockLocation::B1);
        
        // Adapt p: Increase target size for T1, as this block might be useful.
        let delta = if self.b2.len() >= self.b1.len() { 1 } else { self.b1.len() / self.b2.len() };
        self.p = (self.p + delta).min(self.capacity);
        
        self.evict();
      },
      Some(BlockLocation::B2) => {
        self.remove_from_list(&pos, BlockLocation::B2);
        
        // Adapt p: Decrease target size for T1, favoring the frequent list.
        let delta = if self.b1.len() >= self.b2.len() { 1 } else { self.b2.len() / self.b1.len() };
        self.p = self.p.saturating_sub(delta);
        
        self.evict();
      },
      None => {
        // Cold miss
        if self.t1.len() + self.t2.len() >= self.capacity {
          self.evict();
        }
      },
      _ => unreachable!(),
    };
    
    self.block_device.read(buf, pos);
    
    // Ghost hits are "frequent", cold misses are just "recent"
    let add_to_list = if let Some(BlockLocation::B1 | BlockLocation::B2) = location {
      BlockLocation::T2
    } else {
      BlockLocation::T1
    };
    
    self.add_to_mru(&pos, add_to_list);
    let new_block = CacheBlock {
      data: *buf,
      is_dirty: false,
    };
    self.data_store.insert(pos, new_block);
  }
  
  fn write(&mut self, buf: &[u8; block::BLOCK_USIZE], pos: crate::Size) {
    if let Some(block) = self.data_store.get_mut(&pos) {
      // Cache hit
      block.is_dirty = true;
      block.data.copy_from_slice(buf);
      
      // Promote to MRU of T2
      let location = self.page_map.get(&pos).copied().unwrap();
      self.move_page(&pos, location, BlockLocation::T2);
      return;
    }
    // Miss
    
    let location = self.page_map.get(&pos).copied();
    match location {
      Some(BlockLocation::B1) => {
        let delta = if self.b2.len() >= self.b1.len() { 1 } else { self.b1.len() / self.b2.len() };
        self.p = (self.p + delta).min(self.capacity);
        self.evict();
        self.remove_from_list(&pos, BlockLocation::B1);
      },
      Some(BlockLocation::B2) => {
        let delta = if self.b1.len() >= self.b2.len() { 1 } else { self.b2.len() / self.b1.len() };
        self.p = self.p.saturating_sub(delta);
        self.evict();
        self.remove_from_list(&pos, BlockLocation::B2);
      },
      None => {
        // Cold miss
        if self.t1.len() + self.t2.len() >= self.capacity {
          self.evict();
        }
      }
      _ => unreachable!(),
    };
    
    
    // Ghost hits get added to T2; cold misses to T1
    let add_to_list = if let Some(BlockLocation::B1 | BlockLocation::B2) = location {
      BlockLocation::T2
    } else {
      BlockLocation::T1
    };
    
    self.add_to_mru(&pos, add_to_list);
    let new_block = CacheBlock {
      data: *buf,
      is_dirty: true,
    };
    self.data_store.insert(pos, new_block);
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
    let dirty_count = self.data_store.values().filter(|b| b.is_dirty).count();
    writeln!(f, "--- ARC Cache Stats ---")?;
    writeln!(f, "Capacity: {} blocks", self.capacity)?;
    writeln!(f, "Target (p): {}", self.p)?;
    writeln!(f, "T1 (Recent): {} blocks", self.t1.len())?;
    writeln!(f, "T2 (Frequent): {} blocks", self.t2.len())?;
    writeln!(f, "B1 (Ghost Recent): {} blocks", self.b1.len())?;
    writeln!(f, "B2 (Ghost Frequent): {} blocks", self.b2.len())?;
    writeln!(f, "Total Resident: {}", self.data_store.len())?;
    writeln!(f, "Dirty Blocks: {}", dirty_count)?;
    writeln!(f, "--- Underlying Device Stats ---")?;
    self.block_device.stats(f)
  }
}
