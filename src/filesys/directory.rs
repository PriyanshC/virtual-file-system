use std::{
  borrow::{Borrow, BorrowMut},
  cell::RefCell,
};

use crate::{Ofs, Size};

use super::{
  block::BlockDevice,
  free_map::FreeMap,
  inode::{Inode, InodeManager},
  ROOT_INODE,
};

pub const NAME_MAX: usize = 15;
type FileName = [u8; NAME_MAX + 1]; /* Null-terminated */

const NON_ASCII_ERR: &str = "encountered non-ascii character";

pub struct Dir<'a> {
  inode: RefCell<&'a mut Inode>,
}

#[repr(C)]
#[derive(Debug)]
struct DirEntry {
  name: FileName,
  block: Size,
  in_use: bool,
}

impl<'a> Dir<'a> {
  pub fn _create_dir(
    _inodes: &'a mut InodeManager,
    _disk: &'a mut BlockDevice,
    _block: Size,
    _entry_count: Size,
  ) -> Self {
    // inodes.create_inode(length, disk);
    todo!()
  }

  fn init(inode: RefCell<&'a mut Inode>) -> Self {
    Dir { inode }
  }

  pub fn open_root(inodes: &'a mut InodeManager, disk: &mut BlockDevice) -> Self {
    Dir::init(inodes.open_inode(ROOT_INODE, disk))
  }

  pub fn open_path(
    inodes: &'a mut InodeManager,
    disk: &mut BlockDevice,
    _path: &str,
  ) -> Option<Self> {
    // Until nested directories are implemented, we pretend the path is always the root
    Some(Dir::open_root(inodes, disk))
  }

  fn _close(self, inodes: &mut InodeManager) {
    inodes.close(self.inode);
  }

  fn lookup(&self, path: &str, inode_dst: &mut Size, store: bool, disk: &mut BlockDevice) -> bool {
    if path.is_empty() || path.len() > NAME_MAX {
      panic!("should not call this without valid name");
    }

    let inode = self.inode.borrow();

    let mut name = [b'\0'; NAME_MAX + 1];
    for (i, c) in path.chars().enumerate() {
      name[i] = c.try_into().expect(NON_ASCII_ERR);
    }

    let mut start: Ofs = 0;

    while start as usize + std::mem::size_of::<DirEntry>() <= inode.length() as usize {
      let mut raw = [0; std::mem::size_of::<DirEntry>()];
      inode.borrow().read_at(&mut raw, start, disk);

      let entry: DirEntry = unsafe { std::mem::transmute(raw) };
      if entry.in_use && entry.name == name {
        if store {
          *inode_dst = entry.block
        }
        return true;
      }

      start += std::mem::size_of::<DirEntry>() as Ofs;
    }

    false
  }

  pub fn open_file(&self, path: &str, disk: &mut BlockDevice) -> Option<Size> {
    let mut inode = 0;

    if self.lookup(path, &mut inode, true, disk) {
      Some(inode)
    } else {
      None
    }
  }

  pub fn add(
    &mut self,
    path: &str,
    block: Size,
    free_map: &mut FreeMap,
    disk: &mut BlockDevice,
  ) -> bool {
    let mut name = [b'\0'; NAME_MAX + 1];
    for (i, c) in path.chars().enumerate() {
      name[i] = c.try_into().expect(NON_ASCII_ERR);
    }
    
    {
      let inode = self.inode.borrow();

      if path.is_empty() || path.len() > NAME_MAX {
        return false;
      }

      if self.lookup(path, &mut 0, false, disk) {
        return false;
      }

      let mut start: Ofs = 0;
      while start as usize + std::mem::size_of::<DirEntry>() < inode.length() as usize {
        let mut raw = [0; std::mem::size_of::<DirEntry>()];
        inode.borrow().read_at(&mut raw, start, disk);

        let mut entry: DirEntry = unsafe { std::mem::transmute(raw) };

        if !entry.in_use {
          entry.name = name;
          entry.block = block;
          entry.in_use = true;
          let ptr = (&entry) as *const DirEntry as *const u8;
          let buffer: &[u8] =
            unsafe { std::slice::from_raw_parts(ptr, std::mem::size_of::<DirEntry>()) };
          inode.borrow().write_at(buffer, start, disk);
          return true;
        }

        start += std::mem::size_of::<DirEntry>() as Ofs;
      }
    }
    // File full, extend file

    let mut inode = self.inode.borrow_mut();
    let old_len = inode.length();

    inode.borrow_mut().set_len(
      old_len + std::mem::size_of::<DirEntry>() as Size,
      free_map,
      disk,
    );

    let entry = DirEntry {
      name,
      block,
      in_use: true,
    };

    let ptr = (&entry) as *const DirEntry as *const u8;
    let buffer: &[u8] = unsafe { std::slice::from_raw_parts(ptr, std::mem::size_of::<DirEntry>()) };
    inode.borrow().write_at(
      buffer,
      old_len.div_ceil(std::mem::size_of::<DirEntry>() as Size) as Ofs,
      disk,
    );

    true
  }

  pub fn list(&self, disk: &mut BlockDevice) -> Vec<String> {
    let mut files: Vec<String> = Vec::new();

    let mut start: Ofs = 0;
    let inode = self.inode.borrow();

    while start as usize + std::mem::size_of::<DirEntry>() <= inode.length() as usize {
      let mut raw = [0; std::mem::size_of::<DirEntry>()];
      inode.borrow().read_at(&mut raw, start, disk);

      let entry: DirEntry = unsafe { std::mem::transmute(raw) };

      if entry.in_use {
        let terminator = entry
          .name
          .iter()
          .position(|&x| x == b'\0')
          .expect("not null-terminated");
        let filename = String::from_utf8(entry.name[..terminator].to_vec()).expect(NON_ASCII_ERR);
        files.push(filename);
      }

      start += std::mem::size_of::<DirEntry>() as Ofs;
    }

    files
  }
}
