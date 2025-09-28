use core::fmt;

use crate::Size;

pub const BLOCK_SIZE: Size = 1 << 10;
pub const BLOCK_USIZE: usize = BLOCK_SIZE as usize;

pub type Block = [u8; BLOCK_USIZE];
pub const EMPTY_BLOCK: [u8; BLOCK_USIZE] = [0u8; BLOCK_SIZE as usize];

pub struct BlockManager<'a> {
  blocks_by_role: [Option<BlockDevice<'a>>; DeviceType::MaxCount as usize],
}

pub struct BlockDevice<'a> {
  name: &'static str,
  size: Size,
  ops: Box<dyn BlockOperations + 'a>,
  read_count: usize,
  write_count: usize,
  role: DeviceType,
}

#[allow(clippy::type_complexity)]
pub trait BlockOperations {
  fn read(&mut self, buf: &mut [u8; BLOCK_USIZE], pos: Size);
  fn write(&mut self, buf: &[u8; BLOCK_USIZE], pos: Size);
}

#[derive(Clone, PartialEq, Debug)]
pub enum DeviceType {
  Disk,
  MaxCount,
}

impl<'a> BlockManager<'a> {
  pub const fn init() -> Self {
    BlockManager {
      blocks_by_role: [None; DeviceType::MaxCount as usize],
    }
  }

  pub fn get_by_role(&'a mut self, role: DeviceType) -> Option<&'a mut BlockDevice<'a>> {
    assert_ne!(role, DeviceType::MaxCount);
    self.blocks_by_role[role as usize].as_mut()
  }

  pub fn register<B: BlockOperations + 'a>(
    &'a mut self,
    name: &'static str,
    size: Size,
    ops: B,
    role: DeviceType,
  ) {
    assert_ne!(role, DeviceType::MaxCount);

    let idx: usize = role.clone() as usize;
    assert!(self.blocks_by_role[idx].is_none());

    self.blocks_by_role[idx] = Some(BlockDevice {
      name,
      size,
      ops: Box::new(ops),
      read_count: 0,
      write_count: 0,
      role,
    })
  }
}

/*
name: String,
size: Size,
ops: Box<dyn BlockOperations + 'a>,
role: DeviceType, */
impl<'a> BlockDevice<'a> {
  pub fn read(&mut self, buffer: &mut [u8; BLOCK_USIZE], block_num: Size) {
    self.ops.read(buffer, block_num);
    self.read_count += 1;
  }

  pub fn write(&mut self, buffer: &[u8; BLOCK_USIZE], block_num: Size) {
    self.ops.write(buffer, block_num);
    self.write_count += 1;
  }

  pub fn max_size(&self) -> Size {
    self.size
  }
}

impl fmt::Display for BlockDevice<'_> {
  fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
    write!(
      f,
      "Device '{}' assigned to '{:?}' has performed {} read and {} write operations",
      self.name, self.role, self.read_count, self.write_count
    )
  }
}

pub struct CountedBlockOperations<T: BlockOperations> {
  inner: T,
  read_count: usize,
  write_count: usize,
}

impl <T: BlockOperations> BlockOperations for CountedBlockOperations<T> {
    fn read(&mut self, buf: &mut [u8; BLOCK_USIZE], pos: Size) {
      self.read_count += 1;
      self.inner.read(buf, pos);
    }

    fn write(&mut self, buf: &[u8; BLOCK_USIZE], pos: Size) {
      self.write_count += 1;
      self.inner.write(buf, pos);
    }
}
