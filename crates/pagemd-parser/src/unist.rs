//! Original-source coordinates for PageMD's native syntax tree.
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Point {
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}
impl Point {
    pub fn new(line: usize, column: usize, offset: usize) -> Self {
        Self {
            line,
            column,
            offset,
        }
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Position {
    pub start: Point,
    pub end: Point,
}
impl Position {
    pub fn new(
        start_line: usize,
        start_column: usize,
        start_offset: usize,
        end_line: usize,
        end_column: usize,
        end_offset: usize,
    ) -> Self {
        Self {
            start: Point::new(start_line, start_column, start_offset),
            end: Point::new(end_line, end_column, end_offset),
        }
    }
}
/// Byte-based line/column indexing, including CRLF and standalone CR endings.
#[derive(Debug)]
pub struct Location {
    starts: Vec<usize>,
    len: usize,
}
impl Location {
    pub fn new(bytes: &[u8]) -> Self {
        let mut starts = vec![0];
        let mut i = 0;
        while i < bytes.len() {
            match bytes[i] {
                b'\r' => {
                    if bytes.get(i + 1) == Some(&b'\n') {
                        i += 1;
                    }
                    starts.push(i + 1);
                }
                b'\n' => starts.push(i + 1),
                _ => {}
            }
            i += 1;
        }
        Self {
            starts,
            len: bytes.len(),
        }
    }
    pub fn to_point(&self, offset: usize) -> Option<Point> {
        if offset > self.len {
            return None;
        }
        let line = self.starts.partition_point(|start| *start <= offset) - 1;
        Some(Point::new(line + 1, offset - self.starts[line] + 1, offset))
    }
}
