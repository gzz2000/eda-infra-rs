//! Fast Flow: an ultrafast way to parse VCD signal parts.
//!
//! This module provides [`FastFlow`], a Reader-like iterator interface
//! which is very similar to [`vcd_ng::Parser`], but faster.
//!
//! The magic behind this module is the reuse of buffer spaces to
//! represent long signals, and the elimination of temporary memory
//! allocations.
//!
//! Some compatibility has been sacrificed in return for speed. Notably:
//! 0. We only support timestamps and value changes. All other operations
//!    are intentionally ignored and do not produce tokens.
//!    Notably, real values (lines starting with `b'r'`) are ignored.
//! 1. We assert for newline characters after each timestamp `#xxx`
//!    as well as bit change lines.
//!    Previously, both newline and other whitespaces could be used.
//!    This helps us to identify timestamp from the middle of an input.
//! 2. We do not support lines that are itself too long (which means
//!    EXTREMELY long, longer than the whole buffer size).
//!    This should not be a problem with a large buffer like 1MB,
//!    unless one user becomes insane and defines a million-sized
//!    bit vector.

use crate::{ IdCode, InvalidData };
use std::io::{ self, Read };
use linereader::LineReader;

/// An enum of tokens that fast flow supports.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FastFlowToken<'i> {
    Timestamp(u64),
    Value(FFValueChange<'i>)
}

/// A value change token.
///
/// It uses a reference to the internal buffer of the [`FastFlow`]
/// parser object to speed up the retrieval of signals.
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct FFValueChange<'i> {
    /// The symbolic index of the signal to change
    pub id: IdCode,
    /// A byte slice of signal changes. Each byte can be
    /// `0`, `1`, `x`, or `z`.
    pub bits: &'i [u8]
}

/// Fast token stream of timestamp and value changes.
/// See the module-level documentation for details.
pub struct FastFlow<R: Read> {
    /// The line reader inner object
    line_reader: LineReader<R>,
    /// The bytes already read since the start.
    bytes_read: usize
}

impl<R: Read> FastFlow<R> {
    /// Create a new FastFlow from a Read object and a buffer size.
    ///
    /// It is recommended that we do NOT use BufReaders here, because
    /// we have built-in buffers in FastFlow. Doubling the buffer
    /// introduces unnecessary overhead.
    pub fn new(source: R, buf_size: usize) -> FastFlow<R> {
        FastFlow {
            line_reader: LineReader::with_capacity(buf_size, source),
            bytes_read: 0
        }
    }

    /// Get number of bytes that have been read.
    pub fn bytes_read(&self) -> usize {
        self.bytes_read
    }

    /// Read a line.
    /// This records the number of bytes read.
    #[inline]
    pub fn next_line<'i>(&'i mut self) -> io::Result<Option<&'i [u8]>> {
        match self.line_reader.next_line() {
            None => Ok(None),
            Some(Err(e)) => Err(e),
            Some(Ok(line)) => {
                self.bytes_read += line.len();
                Ok(Some(&line[..line.len() - 1]))
            }
        }
    }

    /// Skip a line.
    #[inline]
    pub fn skip_line(&mut self) -> io::Result<()> {
        let _ = self.next_line()?;
        Ok(())
    }

    /// Read a token.
    pub fn next_token<'i>(&'i mut self) -> io::Result<Option<FastFlowToken<'i>>> {
        while let Some(line) = unsafe {
            // The following unsafe transform is NEEDED.
            // If we use &'i mut self here, it will leave a footprint
            // that marks lifetime 'i as *mutated* (exclusively used)
            // inside the loop.
            // As a result, we cannot return the value to outside.
            // This is known as a limitation of current Rust borrow checker.
            // See https://github.com/rust-lang/rust/issues/68117.
            &mut *(self as *mut FastFlow<R>)  // kicks off the lifetime
            // We are safe as long as the reference next_line() returns
            // can outlive 'i **in reality**.
        }.next_line()? {
            let line: &'i [u8] = line;
            // ok, we are safe now with the assumption above.
            
            if line.len() == 0 { continue }
            return Ok(Some(match line[0] {
                b'#' => FastFlowToken::Timestamp(
                    atoi_radix10::parse(&line[1..]).map_err(
                        |_| io::Error::from(InvalidData("parse timestamp failed")))?
                ),
                b'0' | b'1' | b'x' | b'z' => FastFlowToken::Value(
                    FFValueChange { id: IdCode::new(&line[1..])?,
                                    bits: &line[0..1] }
                ),
                b'b' => match line.iter().rposition(|c| *c == b' ') {
                    Some(i) => FastFlowToken::Value(FFValueChange {
                        id: IdCode::new(&line[i + 1..])?,
                        bits: &line[1..i]
                    }),
                    None => return Err(
                        InvalidData("vec value w/o space").into())
                },
                b'r' => continue, // skip real values
                b'$' | b'\t' | b' ' => continue,
                _ => {
                    return Err(InvalidData(
                        "unexpected line in vcd, which is unrecognized \
                         by FastFlow. please try normal parser \
                         instead.").into())
                }
            }))
        }
        Ok(None) // EOF
    }

    /// Read the first complete timestamp token, skipping all other tokens
    /// around it.
    ///
    /// This function looks for a "\n#" pattern. Thus, it will skip at
    /// least one line in the input byte stream.
    pub fn first_timestamp(&mut self) -> io::Result<Option<u64>> {
        self.next_line()?;
        while let Some(line) = self.next_line()? {
            if line.len() == 0 || line[0] != b'#' { continue }
            return Ok(Some(atoi_radix10::parse::<u64>(&line[1..]).map_err(
                |_| io::Error::from(InvalidData("parse first timestamp failed")))?))
        }
        Ok(None) // EOF before even a first timestamp.
    }

    /// Unwraps this `FastFlow` and returns the underlying reader.
    /// All unread buffered lines will be discarded.
    pub fn into_inner(self) -> R {
        self.line_reader.into_inner()
    }
}

#[test]
fn test_fastflow() {
    let buf = br###"
$enddefinitions $end

#0
$dumpvars
0$Q
0"#o
#250
1!
b00000000000000000000000000000000 #0
bxxxx $o
"###;
    let mut f = FastFlow::new(&buf[..], 64);
    assert_eq!(f.first_timestamp().unwrap(), Some(0));
    assert_eq!(f.next_token().unwrap(),
               Some(FastFlowToken::Value(FFValueChange {
                   id: IdCode::new(&b"$Q"[..]).unwrap(), bits: b"0"
               })));
    assert_eq!(f.next_token().unwrap(),
               Some(FastFlowToken::Value(FFValueChange {
                   id: IdCode::new(&b"\"#o"[..]).unwrap(), bits: b"0"
               })));
    assert_eq!(f.next_token().unwrap(),
               Some(FastFlowToken::Timestamp(250)));
    assert_eq!(f.next_token().unwrap(),
               Some(FastFlowToken::Value(FFValueChange {
                   id: IdCode::new(&b"!"[..]).unwrap(), bits: b"1"
               })));
    assert_eq!(f.next_token().unwrap(),
               Some(FastFlowToken::Value(FFValueChange {
                   id: IdCode::new(&b"#0"[..]).unwrap(), bits: b"00000000000000000000000000000000"
               })));
    assert_eq!(f.next_token().unwrap(),
               Some(FastFlowToken::Value(FFValueChange {
                   id: IdCode::new(&b"$o"[..]).unwrap(), bits: b"xxxx"
               })));
    assert_eq!(f.next_token().unwrap(), None);
    assert_eq!(f.bytes_read(), buf.len());
}
