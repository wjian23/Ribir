use std::ops::Range;

use ribir_core::prelude::{AttributedText, CowArc, Substr};
use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};

pub trait BaseText {
  fn measure_bytes(&self, byte_from: usize, char_len: isize) -> usize;
  fn select_token(&self, byte_from: usize) -> Range<usize>;
  fn substr(&self, rg: Range<usize>) -> Substr;
  fn len(&self) -> usize;
  fn is_empty(&self) -> bool { self.len() == 0 }
}

pub trait EditText: BaseText {
  fn insert_str(&mut self, at: usize, s: &str) -> usize;

  fn del_rg_str(&mut self, rg: Range<usize>) -> Range<usize>;
}

fn nearest_char_boundary(text: &str, byte_from: usize) -> usize {
  let byte_from = byte_from.min(text.len());
  if text.is_char_boundary(byte_from) {
    return byte_from;
  }

  let mut boundary = byte_from;
  while boundary > 0 && !text.is_char_boundary(boundary) {
    boundary -= 1;
  }
  boundary
}

impl BaseText for CowArc<str> {
  fn len(&self) -> usize { str::len(self) }
  fn substr(&self, rg: Range<usize>) -> Substr { self.substr(rg) }
  fn measure_bytes(&self, byte_from: usize, char_len: isize) -> usize {
    let byte_from = nearest_char_boundary(self, byte_from);
    let mut len = char_len.abs();
    let is_backward = char_len > 0;

    let mut legacy = GraphemeCursor::new(byte_from, self.len(), true);
    while len > 0 {
      len -= 1;
      let res =
        if is_backward { legacy.next_boundary(self, 0) } else { legacy.prev_boundary(self, 0) };
      if res.unwrap().is_none() {
        break;
      }
    }
    if is_backward { legacy.cur_cursor() - byte_from } else { byte_from - legacy.cur_cursor() }
  }

  fn select_token(&self, byte_from: usize) -> Range<usize> {
    if byte_from >= self.len() {
      return Range { start: self.len(), end: self.len() };
    }
    let byte_from = nearest_char_boundary(self, byte_from);
    let mut legacy = GraphemeCursor::new(byte_from, self.len(), true);
    let is_whitespace = self[byte_from..]
      .chars()
      .next()
      .unwrap()
      .is_whitespace();
    loop {
      let size = legacy.prev_boundary(self, 0).unwrap();
      if size.is_none() || size.unwrap() == 0 {
        break;
      }
      let pos = legacy.cur_cursor();
      let c = self[pos..].chars().next().unwrap();
      if is_whitespace != c.is_whitespace() || c == '\r' || c == '\n' {
        break;
      }
    }

    let mut base = legacy.cur_cursor();
    let it = self[legacy.cur_cursor()..].split_word_bounds();
    for word in it {
      if base + word.len() > byte_from {
        return Range { start: base, end: base + word.len() };
      }
      base += word.len();
    }
    Range { start: self.len(), end: self.len() }
  }
}

impl EditText for CowArc<str> {
  fn insert_str(&mut self, at: usize, v: &str) -> usize {
    if !v.is_empty() {
      let mut s = self.to_string();
      s.insert_str(at, v);
      *self = s.into();
    }
    v.len()
  }

  fn del_rg_str(&mut self, mut rg: Range<usize>) -> Range<usize> {
    rg.start = rg.start.min(self.len());
    rg.end = rg.end.min(self.len());

    if !rg.is_empty() {
      let mut s = self.to_string();
      s.drain(rg.clone());
      *self = s.into();
    }
    rg
  }
}

impl BaseText for AttributedText {
  fn len(&self) -> usize { self.len_bytes() }

  fn substr(&self, rg: Range<usize>) -> Substr { self.text.substr(rg) }

  fn measure_bytes(&self, byte_from: usize, char_len: isize) -> usize {
    self.text.measure_bytes(byte_from, char_len)
  }

  fn select_token(&self, byte_from: usize) -> Range<usize> { self.text.select_token(byte_from) }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn select_token_accepts_non_char_boundary_index() {
    let text: CowArc<str> = "234 浮动洒".into();

    assert!(
      !text
        .substr(text.select_token(12))
        .to_string()
        .is_empty()
    );
    assert!(
      !text
        .substr(text.select_token(11))
        .to_string()
        .is_empty()
    );
  }
}
