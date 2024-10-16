use crate::{Ofs, Size};
use block::{BlockManager, DeviceType};
use directory::Dir;
use free_map::FreeMap;
use inode::InodeManager;
use std::borrow::BorrowMut;
use vdisk::VDisk;
use vfile::VFile;

mod block;
mod directory;
mod free_map;
mod inode;
mod vdisk;
mod vfile;

pub struct Filesys<'a> {
  inodes: InodeManager,
  block_devs: BlockManager<'a>,
  free_map: Option<FreeMap<'a>>,
}

const ROOT_INODE: Size = 0;
const FREE_MAP_INODE: Size = 1;

impl<'a> Filesys<'a> {
  pub const fn init() -> Self {
    let inodes = InodeManager::init();
    let block_devs = BlockManager::init();

    Filesys {
      inodes,
      block_devs,
      free_map: None,
    }
  }

  pub fn new_disk(&'a mut self, host_path: &str, disk_block_count: Size) {
    let vdisk = VDisk::new(host_path, disk_block_count);

    self
      .block_devs
      .register("DISK", disk_block_count, vdisk, DeviceType::Disk);
  }

  pub fn init_free_map(&'a mut self) {
    let disk = self
      .block_devs
      .get_by_role(DeviceType::Disk)
      .expect("no disk found");

    let block_count = disk.max_size();

    let inode = self.inodes.open_inode(FREE_MAP_INODE, disk);
    let file = VFile::open(inode);

    self.free_map = Some(FreeMap::init(file, block_count));
  }

  pub fn load_disk(&'a mut self, host_path: &str) {
    let (vdisk, disk_block_count) = VDisk::identify(host_path);

    self
      .block_devs
      .register("DISK", disk_block_count, vdisk, DeviceType::Disk);

    todo!()
  }

  pub fn create_file(&'a mut self, path: &str, length: Size) -> bool {
    let disk = self
      .block_devs
      .get_by_role(DeviceType::Disk)
      .expect("no disk found");

    let inode = self.inodes.borrow_mut().create_inode(
      length,
      disk,
      self.free_map.as_mut().expect("free map not initialised"),
    );
    let inumber = inode.borrow().inumber();

    let mut dir = Dir::open_root(&mut self.inodes, disk);

    dir.add(path, inumber, self.free_map.as_mut().expect("msg"), disk)
  }

  pub fn open_file(&'a mut self, path: &str) -> Option<VFile<'a>> {
    let disk = self
      .block_devs
      .get_by_role(DeviceType::Disk)
      .expect("no disk found");

    let dir = Dir::open_root(&mut self.inodes, disk);
    dir
      .open_file(path, disk)
      .map(|i| VFile::open(self.inodes.open_inode(i, disk)))
  }

  pub fn file_read(&'a mut self, file: &mut VFile, buffer: &mut [u8], offset: Ofs) -> Ofs {
    let disk = self
      .block_devs
      .get_by_role(DeviceType::Disk)
      .expect("no disk found");

    file.read(buffer, offset, disk)
  }

  pub fn file_write(&'a mut self, file: &mut VFile, buffer: &[u8], offset: Ofs) -> Ofs {
    let disk = self
      .block_devs
      .get_by_role(DeviceType::Disk)
      .expect("no disk found");

    file.write(buffer, offset, disk)
  }

  pub fn _remove_file(&mut self, _path: &str) -> bool {
    todo!()
  }

  pub fn display_disk_stats(&'a mut self) {
    let disk = self
      .block_devs
      .get_by_role(DeviceType::Disk)
      .expect("no disk found");
    
    println!("{}", disk);
  }
}
