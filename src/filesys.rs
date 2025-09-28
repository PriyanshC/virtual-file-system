use crate::{filesys::vdisk::buffer_cache::ArcCacheDisk, Ofs, Size};
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

pub enum BufferCacheStrategy {
  None,
  Arc { capacity: usize },
}

const ROOT_INODE: Size = 0;
const FREE_MAP_INODE: Size = 1;

const NO_DISK_ERR: &str = "disk not found";
const NO_FREE_MAP_ERR: &str = "free map not initialised";

impl<'a> Filesys<'a> {
  
  /*
    Initialisation
  */

  pub const fn init() -> Self {
    Filesys {
      inodes: InodeManager::init(),
      block_devs: BlockManager::init(),
      free_map: None,
    }
  }

  pub fn new_disk(&'a mut self, host_path: &str, disk_block_count: Size, cache_strategy: BufferCacheStrategy) {
    let vdisk = VDisk::new(host_path, disk_block_count);
    match cache_strategy {
      BufferCacheStrategy::None => {
        self
          .block_devs
          .register("DISK", disk_block_count, vdisk, DeviceType::Disk);
        },
        BufferCacheStrategy::Arc { capacity } => {
          let disk = ArcCacheDisk::new(vdisk, capacity);
          self
            .block_devs
          .register("DISK", disk_block_count, disk, DeviceType::Disk);
        },
    }
  }

  pub fn load_disk(&'a mut self, host_path: &str) {
    let (vdisk, disk_block_count) = VDisk::identify(host_path);

    self
      .block_devs
      .register("DISK", disk_block_count, vdisk, DeviceType::Disk);

    todo!("ensure free map reads from disk")
  }

  pub fn init_free_map(&'a mut self) {
    let disk = self
      .block_devs
      .get_by_role(DeviceType::Disk)
      .expect(NO_DISK_ERR);

    let block_count = disk.max_size();

    let inode = self.inodes.open_inode(FREE_MAP_INODE, disk);
    let file = VFile::open(inode);

    self.free_map = Some(FreeMap::init(file, block_count));
  }

  /*
    File operations
  */

  pub fn create_file(&'a mut self, path: &str, length: Size) -> bool {
    let disk = self
      .block_devs
      .get_by_role(DeviceType::Disk)
      .expect(NO_DISK_ERR);

    let free_map = self.free_map.as_mut().expect(NO_FREE_MAP_ERR);

    let inode = self
      .inodes
      .borrow_mut()
      .create_inode(length, disk, free_map);
    let inumber = inode.borrow().inumber();

    if let Some(mut dir) = Dir::open_path(&mut self.inodes, disk, path) {
      dir.add(path, inumber, free_map, disk)
    } else {
      false
    }
  }

  pub fn open_file(&'a mut self, path: &str) -> Option<VFile<'a>> {
    let disk = self
      .block_devs
      .get_by_role(DeviceType::Disk)
      .expect(NO_DISK_ERR);

    let dir = Dir::open_path(&mut self.inodes, disk, path)?;
    dir
      .open_file(path, disk)
      .map(|i| VFile::open(self.inodes.open_inode(i, disk)))
  }

  pub fn file_read(&'a mut self, file: &mut VFile, buffer: &mut [u8], offset: Ofs) -> Ofs {
    let disk = self
      .block_devs
      .get_by_role(DeviceType::Disk)
      .expect(NO_DISK_ERR);

    file.read(buffer, offset, disk)
  }

  pub fn file_write(&'a mut self, file: &mut VFile, buffer: &[u8], offset: Ofs) -> Ofs {
    let disk = self
      .block_devs
      .get_by_role(DeviceType::Disk)
      .expect(NO_DISK_ERR);

    file.write(buffer, offset, disk)
  }

  pub fn _remove_file(&mut self, _path: &str) -> bool {
    todo!()
  }

  /*
    Directory operations
  */

  pub fn list(&'a mut self, path: &str) -> Option<Vec<String>> {
    let disk = self
      .block_devs
      .get_by_role(DeviceType::Disk)
      .expect(NO_DISK_ERR);

    let dir = Dir::open_path(&mut self.inodes, disk, path)?;
    Some(dir.list(disk))
  }

  /*
    Misc operations
  */

  pub fn display_disk_stats(&'a mut self) {
    let disk = self
      .block_devs
      .get_by_role(DeviceType::Disk)
      .expect(NO_DISK_ERR);

    println!("{}", disk);
  }
}
