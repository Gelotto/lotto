pub fn hash_numbers(numbers: &Vec<u16>) -> String {
  let parts: Vec<String> = numbers.iter().map(|n| n.to_string()).collect();
  parts.join(":")
}
