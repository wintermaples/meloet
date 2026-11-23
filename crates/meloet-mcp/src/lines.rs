//! One message a line, read with a limit. Each stream has one reader for as long as it is
//! read, so nothing it has taken in ahead is lost.

use std::io::{self, BufRead};

/// A longer line is thrown away whole.
pub const MAX_LINE: usize = 16 << 20;

#[derive(Debug, PartialEq, Eq)]
pub enum Line {
    Text(String),
    /// Longer than the limit, or not UTF-8: skipped.
    Skipped,
    End,
}

pub fn read_line(reader: &mut impl BufRead, max: usize) -> io::Result<Line> {
    let mut buf = Vec::new();
    let mut too_long = false;
    loop {
        let available = match reader.fill_buf() {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(e),
        };
        if available.is_empty() {
            return Ok(match (buf.is_empty(), too_long) {
                (true, false) => Line::End,
                (_, true) => Line::Skipped,
                (false, false) => text(buf),
            });
        }
        let (part, used, ended) = match available.iter().position(|&b| b == b'\n') {
            Some(i) => (&available[..i], i + 1, true),
            None => (available, available.len(), false),
        };
        if !too_long {
            if buf.len() + part.len() > max {
                too_long = true;
                buf = Vec::new();
            } else {
                buf.extend_from_slice(part);
            }
        }
        reader.consume(used);
        if ended {
            return Ok(if too_long { Line::Skipped } else { text(buf) });
        }
    }
}

fn text(mut buf: Vec<u8>) -> Line {
    if buf.last() == Some(&b'\r') {
        buf.pop();
    }
    String::from_utf8(buf).map_or(Line::Skipped, Line::Text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lines_come_whole_and_in_order() {
        let mut r = io::BufReader::with_capacity(4, &b"{\"a\":1}\r\n{\"b\":2}\nlast"[..]);
        assert_eq!(
            read_line(&mut r, 100).unwrap(),
            Line::Text("{\"a\":1}".into())
        );
        assert_eq!(
            read_line(&mut r, 100).unwrap(),
            Line::Text("{\"b\":2}".into())
        );
        assert_eq!(read_line(&mut r, 100).unwrap(), Line::Text("last".into()));
        assert_eq!(read_line(&mut r, 100).unwrap(), Line::End);
    }

    #[test]
    fn a_long_line_is_skipped_and_the_next_one_read() {
        let mut input = vec![b'x'; 50];
        input.extend_from_slice(b"\nok\n");
        let mut r = io::BufReader::with_capacity(8, &input[..]);
        assert_eq!(read_line(&mut r, 20).unwrap(), Line::Skipped);
        assert_eq!(read_line(&mut r, 20).unwrap(), Line::Text("ok".into()));
        assert_eq!(read_line(&mut r, 20).unwrap(), Line::End);
    }

    #[test]
    fn not_utf8_is_skipped() {
        let mut r = io::BufReader::new(&b"\xff\xfe\nok\n"[..]);
        assert_eq!(read_line(&mut r, 20).unwrap(), Line::Skipped);
        assert_eq!(read_line(&mut r, 20).unwrap(), Line::Text("ok".into()));
    }
}
