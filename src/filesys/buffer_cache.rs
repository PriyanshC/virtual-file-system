use crate::filesys::block;

pub trait BufferCache {
  fn flush(&mut self);
}

pub struct ClockBufferCacheDisk {

}


impl BufferCache for ClockBufferCacheDisk {
    fn flush(&mut self) {
        todo!()
    }
}