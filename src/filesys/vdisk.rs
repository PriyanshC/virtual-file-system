use super::block;
use crate::Size;

use std::{
  fs::File,
  io::{Read, Seek, Write},
};

pub struct VDisk {
  host: File,
}

impl VDisk {
  pub fn new(host_path: &str, disk_block_count: Size) -> Self {
    let host = File::options()
      .write(true)
      .read(true)
      .create_new(true)
      .open(host_path)
      .expect("could not open host file");

    host
      .set_len(disk_block_count * block::BLOCK_SIZE)
      .expect("unable to set host file size");
    VDisk { host }
  }

  pub fn identify(host_path: &str) -> (Self, Size) {
    let host = File::options()
      .write(true)
      .read(true)
      .open(host_path)
      .expect("could not open host file");

    let host_size = host.metadata().expect("unable to query metadata").len();
    assert_eq!(host_size % block::BLOCK_SIZE, 0);

    (VDisk { host }, host_size / block::BLOCK_SIZE)
  }
}

impl block::BlockOperations for VDisk {
  fn read(&mut self, buf: &mut [u8; block::BLOCK_USIZE], pos: Size) {
    #[cfg(debug_assertions)]
    println!("Disk reading block {}", pos);

    self
      .host
      .seek(std::io::SeekFrom::Start(pos * block::BLOCK_SIZE))
      .expect("could not seek file");
    self
      .host
      .read_exact(buf)
      .expect("could not read all bytes to buffer");
    self.host.flush().expect("could not flush file contents");
    self.host.sync_all().expect("could not flush file contents");
  }

  fn write(&mut self, buf: &[u8; block::BLOCK_USIZE], pos: Size) {
    #[cfg(debug_assertions)]
    println!("Disk writing block {}", pos);

    self
      .host
      .seek(std::io::SeekFrom::Start(pos * block::BLOCK_SIZE))
      .expect("could not seek file");
    self
      .host
      .write_all(buf)
      .expect("could not write all bytes to file");
    self.host.flush().expect("could not flush file contents");
    self.host.sync_all().expect("could not flush file contents");

    #[cfg(debug_assertions)]
    {
      let mut temp = [u8::MAX; block::BLOCK_USIZE];
      self.read(&mut temp, pos);
      assert_eq!(temp, *buf);
    };
  }
}
