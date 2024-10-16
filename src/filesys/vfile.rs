use std::cell::RefCell;

use crate::{Ofs, Size};

use super::{
  block::BlockDevice,
  inode::{Inode, InodeManager},
};

pub struct VFile<'a> {
  pos: Ofs,
  inode: RefCell<&'a mut Inode>,
}

impl<'a> VFile<'a> {
  pub fn open(inode: RefCell<&'a mut Inode>) -> Self {
    VFile { pos: 0, inode }
  }

  pub fn close(self, inodes: &mut InodeManager) {
    inodes.close(self.inode);
  }

  pub fn read(&mut self, buffer: &mut [u8], offset: Ofs, disk: &mut BlockDevice) -> Ofs {
    let bytes_read = self
      .inode
      .borrow_mut()
      .read_at(buffer, self.pos + offset, disk);

    self.seek(bytes_read);
    bytes_read
  }

  pub fn write(&mut self, buffer: &[u8], offset: Ofs, disk: &mut BlockDevice) -> Ofs {
    let bytes_written = self
      .inode
      .borrow_mut()
      .write_at(buffer, self.pos + offset, disk);

    self.seek(bytes_written);
    bytes_written
  }

  pub fn length(&self) -> Size {
    self.inode.borrow().length()
  }

  pub fn seek_start(&mut self) {
    self.pos = 0;
  }

  pub fn seek(&mut self, offset: Ofs) {
    self.pos += offset;
  }

  pub fn tell(&self) -> Ofs {
    self.pos
  }

  pub fn compare(&self, other: &VFile) -> bool {
    self.inode.borrow().inumber() == other.inode.borrow().inumber()
  }
}
