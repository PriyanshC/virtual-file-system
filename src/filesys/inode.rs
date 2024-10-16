use std::cell::RefCell;

use super::{
  block::{self, BlockDevice, BLOCK_USIZE},
  free_map::FreeMap,
};
use crate::{Ofs, Size};

const INODE_MAGIC: Size = 0x8BCEFADC;

const N_DIRECT: usize = 4;
const N_INDIRECT: usize = 1;
const N_DOUBLY_INDIRECT: usize = 1;

const PTRS_PER_BLOCK: usize = block::BLOCK_USIZE / std::mem::size_of::<Size>();
type PtrBlock = [Size; PTRS_PER_BLOCK];

pub struct InodeManager {
  open_list: Vec<Inode>,
}

/* In-memory Inode */
pub struct Inode {
  open_count: usize,
  block: Size,
  data: InodeDisk,
}

/* On-disk Inode must be exactly BLOCK_SIZE bytes long */
#[repr(C)]
#[derive(Clone, Debug)]
struct InodeDisk {
  direct: [Size; N_DIRECT],
  indirect: [Size; N_INDIRECT],
  doubly_indirect: [Size; N_DOUBLY_INDIRECT],
  magic: Size,
  len: Size,
  unused: [u8;
    BLOCK_USIZE - std::mem::size_of::<Size>() * (2 + N_DIRECT + N_INDIRECT + N_DOUBLY_INDIRECT)],
}

impl InodeManager {
  pub const fn init() -> Self {
    Self {
      open_list: Vec::new(),
    }
  }

  pub fn create_inode(
    &mut self,
    length: Size,
    disk: &mut BlockDevice,
    free_map: &mut FreeMap,
  ) -> RefCell<&mut Inode> {
    assert_eq!(std::mem::size_of::<InodeDisk>(), block::BLOCK_USIZE);

    let block_count = length.div_ceil(block::BLOCK_SIZE) as usize;

    let mut allocations: Vec<Size> = Vec::new();
    free_map.allocate(1 + block_count, &mut allocations);
    let mut blocks = allocations.into_iter();

    let inode_block = blocks.next().expect("block not found");

    /* Allocate disk blocks */
    let mut skip = 0;
    let mut data = InodeDisk::default();
    fill_direct(&mut skip, &mut data.direct, &mut blocks);
    fill_indirect(&mut skip, &mut data.indirect, &mut blocks, disk);
    fill_doubly_indirect(&mut skip, &mut data.indirect, &mut blocks, disk);
    data.len = length;

    /* Write to disk */
    disk.write(&data.clone().into(), inode_block);

    /* Initial mem */
    let inode = Inode {
      open_count: 1,
      block: inode_block,
      data,
    };

    /* Push to global list */
    self.open_list.push(inode);

    let inode = self
      .open_list
      .iter_mut()
      .find(|i| i.block == inode_block)
      .expect("2");
    RefCell::new(inode)
  }

  pub fn open_inode(&mut self, block_num: Size, disk: &mut BlockDevice) -> RefCell<&mut Inode> {
    let idx: usize = if let Some(i) = self.open_list.iter().position(|i| i.block == block_num) {
      i
    } else {
      let mut block = block::EMPTY_BLOCK;
      disk.read(&mut block, block_num);
      let data: InodeDisk = unsafe { std::mem::transmute(block) };

      let inode = Inode {
        open_count: 0,
        data,
        block: block_num,
      };
      let i = self.open_list.len();
      self.open_list.push(inode);
      i
    };

    let inode = self.open_list.get_mut(idx).expect("msg");
    inode.incr_open();
    RefCell::new(inode)
  }

  pub fn close(&mut self, inode_ref: RefCell<&mut Inode>) {
    let mut inode = inode_ref.borrow_mut();
    inode.decr_open();

    if inode.no_refs() {
      let idx: usize = self
        .open_list
        .iter()
        .position(|i| i.block == inode.block)
        .expect("internal error: inode not found");
      self.open_list.swap_remove(idx);
    };
  }
}

impl Inode {
  pub fn length(&self) -> Size {
    self.data.len
  }

  pub fn inumber(&self) -> Size {
    self.block
  }

  fn incr_open(&mut self) {
    self.open_count += 1
  }

  fn decr_open(&mut self) {
    self.open_count -= 1
  }

  fn no_refs(&self) -> bool {
    self.open_count == 0
  }

  pub fn read_at(&self, buffer: &mut [u8], offset: Ofs, disk: &mut BlockDevice) -> Ofs {
    let mut size = buffer.len() as Ofs;
    let mut ofs = offset;
    let mut bytes_written: Ofs = 0;

    #[cfg(debug_assertions)]
    println!(
      "Inode {} reading {} bytes at {}",
      self.inumber(),
      buffer.len(),
      offset
    );

    let mut blocks = self
      .data
      .block_range(buffer.len().try_into().unwrap(), offset, disk)
      .into_iter();

    let mut buf: *mut u8 = buffer.as_mut_ptr();

    while size > 0 {
      let block_ofs = ofs % block::BLOCK_SIZE as Ofs;
      let block_idx = blocks.next().expect("block not found");

      let inode_left = self.length() as Ofs - ofs;
      let block_left = block::BLOCK_SIZE as Ofs - block_ofs;
      let min_left = std::cmp::min(inode_left, block_left);

      let chunk_size = std::cmp::min(size, min_left);

      if chunk_size <= 0 {
        return bytes_written;
      }

      /* Bounce buffer */
      let mut bounce = block::EMPTY_BLOCK;
      disk.read(&mut bounce, block_idx);
      unsafe {
        bounce
          .as_mut_ptr()
          .add(block_ofs as _)
          .copy_to_nonoverlapping(buf, chunk_size as usize);
      };

      /* Advance */
      buf = unsafe { buf.add(chunk_size.try_into().expect("msg")) };
      size -= chunk_size;
      ofs += chunk_size;
      bytes_written += chunk_size;
    }

    bytes_written
  }

  pub fn write_at(&self, buffer: &[u8], offset: Ofs, disk: &mut BlockDevice) -> Ofs {
    let mut size = buffer.len() as Ofs;
    let mut ofs = offset;
    let mut bytes_written: Ofs = 0;

    #[cfg(debug_assertions)]
    println!(
      "Inode {} writing {} bytes at {}",
      self.inumber(),
      buffer.len(),
      offset
    );

    let mut blocks = self
      .data
      .block_range(buffer.len().try_into().unwrap(), offset, disk)
      .into_iter();

    let mut buf: *const u8 = buffer.as_ptr();

    while size > 0 {
      let block_ofs = ofs % block::BLOCK_SIZE as Ofs;
      let block_idx = blocks.next().expect("block not found");

      let inode_left = self.length() as Ofs - ofs;
      let block_left = block::BLOCK_SIZE as Ofs - block_ofs;
      let min_left = std::cmp::min(inode_left, block_left);

      let chunk_size = std::cmp::min(size, min_left);

      if chunk_size <= 0 {
        return bytes_written;
      }

      /* Bounce buffer */

      let mut bounce = block::EMPTY_BLOCK;
      disk.read(&mut bounce, block_idx);
      unsafe {
        buf.copy_to(bounce.as_mut_ptr().add(block_ofs as usize), chunk_size as _);
      };

      disk.write(&bounce, block_idx);

      /* Advance */
      buf = unsafe { buf.add(chunk_size.try_into().expect("msg")) };
      size -= chunk_size;
      ofs += chunk_size;
      bytes_written += chunk_size;
    }

    bytes_written
  }

  pub fn set_len(&mut self, len: Size, free_map: &mut FreeMap, disk: &mut BlockDevice) {
    let cur_block_count = self.length().div_ceil(block::BLOCK_SIZE) as usize;
    let req_block_count = len.div_ceil(block::BLOCK_SIZE) as usize;

    if cur_block_count <= req_block_count {
      self.data.len = len;
      return;
    }

    let mut allocations: Vec<Size> = Vec::new();
    free_map.allocate(req_block_count - cur_block_count, &mut allocations);
    let mut blocks = allocations.into_iter();

    let mut skip = cur_block_count;
    fill_direct(&mut skip, &mut self.data.direct, &mut blocks);
    fill_indirect(&mut skip, &mut self.data.direct, &mut blocks, disk);
    fill_doubly_indirect(&mut skip, &mut self.data.direct, &mut blocks, disk);
    self.data.len = len;

    let buffer: block::Block = unsafe { std::mem::transmute(self.data.clone()) };
    disk.write(&buffer, self.inumber());
  }
}

impl Default for InodeDisk {
  fn default() -> Self {
    InodeDisk {
      direct: [0; N_DIRECT],
      indirect: [0; N_INDIRECT],
      doubly_indirect: [0; N_DOUBLY_INDIRECT],
      magic: INODE_MAGIC,
      len: 0,
      unused: [0; BLOCK_USIZE
        - std::mem::size_of::<Size>() * (2 + N_DIRECT + N_INDIRECT + N_DOUBLY_INDIRECT)],
    }
  }
}

impl From<block::Block> for InodeDisk {
  fn from(block: block::Block) -> Self {
    unsafe { std::mem::transmute(block) }
  }
}

impl From<InodeDisk> for block::Block {
  fn from(data: InodeDisk) -> Self {
    unsafe { std::mem::transmute(data) }
  }
}

impl InodeDisk {
  fn direct_range(
    mut skip: usize,
    mut count: Size,
    direct: &[Size],
    blocks: &mut Vec<Size>,
  ) -> (usize, Size) {
    let mut direct_count = 0;
    while count > 0 && direct_count < direct.len() {
      if skip > 0 {
        skip -= 1;
        direct_count += 1;
        continue;
      }

      blocks.push(direct[direct_count]);
      direct_count += 1;
      count -= 1;
    }

    (skip, count)
  }

  fn indirect_range(
    mut skip: usize,
    mut count: Size,
    indirect: &[Size],
    blocks: &mut Vec<Size>,
    disk: &mut BlockDevice,
  ) -> (usize, Size) {
    let mut indirect_count = 0;
    while count > 0 && indirect_count < indirect.len() {
      let mut indirect_block_raw = block::EMPTY_BLOCK;
      disk.read(&mut indirect_block_raw, indirect[indirect_count]);

      let indirect_block: PtrBlock = unsafe { std::mem::transmute(indirect_block_raw) };

      (skip, count) = InodeDisk::direct_range(skip, count, &indirect_block, blocks);

      indirect_count += 1;
    }

    (skip, count)
  }

  fn doubly_indirect_range(
    mut skip: usize,
    mut count: Size,
    doubly_indirect: &[Size],
    blocks: &mut Vec<Size>,
    disk: &mut BlockDevice,
  ) -> (usize, Size) {
    let mut doubly_indirect_count = 0;
    while count > 0 && doubly_indirect_count < doubly_indirect.len() {
      let mut doubly_indirect_block_raw = block::EMPTY_BLOCK;
      disk.read(
        &mut doubly_indirect_block_raw,
        doubly_indirect[doubly_indirect_count],
      );

      let doubly_indirect_block: PtrBlock =
        unsafe { std::mem::transmute(doubly_indirect_block_raw) };

      (skip, count) = InodeDisk::indirect_range(skip, count, &doubly_indirect_block, blocks, disk);

      doubly_indirect_count += 1;
    }

    (skip, count)
  }

  fn block_range(&self, buf_len: Size, offset: Ofs, disk: &mut BlockDevice) -> Vec<Size> {
    let mut skip = offset as usize / block::BLOCK_USIZE;
    let mut count = buf_len.div_ceil(block::BLOCK_SIZE);

    let mut blocks: Vec<Size> = Vec::new();

    (skip, count) = InodeDisk::direct_range(skip, count, &self.direct, &mut blocks);
    (skip, count) = InodeDisk::indirect_range(skip, count, &self.indirect, &mut blocks, disk);
    _ = InodeDisk::doubly_indirect_range(skip, count, &self.doubly_indirect, &mut blocks, disk);

    blocks
  }
}

fn fill_direct(skip: &mut usize, dst: &mut [Size], blocks: &mut impl Iterator<Item = Size>) {
  for elem in dst {
    if *skip > 0 {
      *skip -= 1;
    } else if let Some(block) = blocks.next() {
      *elem = block;
    } else {
      return;
    }
  }
}

fn fill_indirect(
  skip: &mut usize,
  dst: &mut [Size],
  blocks: &mut impl Iterator<Item = Size>,
  disk: &mut BlockDevice,
) {
  for ptr in dst {
    if let Some(block) = blocks.next() {
      *ptr = block;
      let mut direct_block: PtrBlock = [0; PTRS_PER_BLOCK];
      fill_direct(skip, &mut direct_block, blocks);

      let raw: block::Block = unsafe { std::mem::transmute_copy(&direct_block) };
      disk.write(&raw, block);
    } else {
      return;
    }
  }
}

fn fill_doubly_indirect(
  skip: &mut usize,
  dst: &mut [Size],
  blocks: &mut impl Iterator<Item = Size>,
  disk: &mut BlockDevice,
) {
  for ptr in dst {
    if let Some(block) = blocks.next() {
      *ptr = block;
      let mut indirect_block: PtrBlock = [0; PTRS_PER_BLOCK];
      fill_indirect(skip, &mut indirect_block, blocks, disk);

      let raw: block::Block = unsafe { std::mem::transmute_copy(&indirect_block) };
      disk.write(&raw, block);
    } else {
      return;
    }
  }
}

fn _bytes_to_blocks(bytes: Size) -> Size {
  bytes.div_ceil(block::BLOCK_SIZE)
}
