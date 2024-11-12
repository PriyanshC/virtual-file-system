use super::vfile::VFile;
use crate::bitmap::Bitmap;

use crate::Size;

pub struct FreeMap<'a> {
  _file: VFile<'a>,
  bitmap: Bitmap,
}

impl<'a> FreeMap<'a> {
  pub fn init(_file: VFile<'a>, bits: Size) -> Self {
    let mut bitmap = Bitmap::new(bits);
    bitmap.mark(super::ROOT_INODE);
    bitmap.mark(super::FREE_MAP_INODE);
    FreeMap { _file, bitmap }
  }

  fn _open() -> Self {
    // Read bitmap from file
    // let _file = VFile::open(Inode::open(FREE_MAP_BLOCK));
    todo!()
  }

  pub fn allocate(&mut self, blocks: usize, dst: &mut Vec<Size>) -> bool {
    let mut allocations: Vec<Size> = Vec::with_capacity(blocks);

    let mut idx = 0;
    let mut count = 0;

    while count < blocks && idx < self.bitmap.count() {
      if self.bitmap.compare_and_flip(idx) {
        count += 1;
        allocations.push(idx);
      }
      idx += 1;
    }

    if count == blocks {
      allocations.into_iter().for_each(|a| dst.push(a));
      true
    } else {
      false
    }
  }

  fn _release(&mut self, block: Size) {
    assert!(!self.bitmap.test(block));
    self.bitmap.reset(block);
  }
}
