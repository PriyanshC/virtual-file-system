use crate::Size;

type Elem = u32;
const ELEM_BITS: Size = (std::mem::size_of::<Elem>() as Size) * 8;

pub struct Bitmap {
  count: Size,
  elems: Vec<Elem>,
}

fn byte_count(count: Size) -> usize {
  (count as usize).div_ceil(std::mem::size_of::<Elem>())
}

fn byte_index(bit: Size) -> usize {
  (bit / ELEM_BITS) as usize
}

fn elem_mask(bit: Size) -> Elem {
  1 << (bit % ELEM_BITS)
}

impl Bitmap {
  pub fn new(count: Size) -> Self {
    Bitmap {
      count,
      elems: vec![0; byte_count(count)],
    }
  }

  pub fn count(&self) -> Size {
    self.count
  }

  pub fn test(&self, bit: Size) -> bool {
    assert!(bit < self.count);

    let idx = byte_index(bit);
    let mask = elem_mask(bit);

    let byte = self.elems.get(idx).expect("internal err");
    *byte & mask != 0
  }

  pub fn set(&mut self, bit: Size, value: bool) {
    assert!(bit < self.count);

    if value {
      self.mark(bit);
    } else {
      self.reset(bit);
    }
  }

  pub fn mark(&mut self, bit: Size) {
    assert!(bit < self.count);

    let idx = byte_index(bit);
    let mask = elem_mask(bit);

    let byte = self.elems.get_mut(idx).expect("internal err");
    *byte |= mask;
  }

  pub fn reset(&mut self, bit: Size) {
    assert!(bit < self.count);

    let idx = byte_index(bit);
    let mask = elem_mask(bit);

    let byte = self.elems.get_mut(idx).expect("internal err");
    *byte &= !mask;
  }

  pub fn compare_and_flip(&mut self, bit: Size) -> bool {
    if !self.test(bit) {
      self.mark(bit);
      true
    } else {
      false
    }
  }
}
