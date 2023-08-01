pub struct Xorshift32 {
  state: u32,
}

impl Xorshift32 {
  pub fn new(seed: u32) -> Self {
    Self { state: seed }
  }

  fn next(&mut self) -> u32 {
    let mut x = self.state;
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    self.state = x;
    x
  }

  pub fn random_int_in_range(
    &mut self,
    min: u32,
    max: u32,
  ) -> u32 {
    // Make sure min and max are in the correct order
    let (min, max) = if min <= max { (min, max) } else { (max, min) };
    let range = max - min;

    if range == 0 {
      return min;
    }

    let random_value = self.next();
    let scaled_value = random_value % (range + 1);
    scaled_value + min
  }
}
