// Copyright 2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use prelude::*;

use comm::{Port, Chan};
use cmp;
use io;
use option::{None, Option, Some};
use super::{Reader, Writer};
use vec::{bytes, CopyableVector, MutableVector, ImmutableVector};

/// Allows reading from a port.
///
/// # Example
///
/// ```
/// let reader = PortReader::new(port);
///
/// let mut buf = ~[0u8, ..100];
/// match reader.read(buf) {
///     Some(nread) => println!("Read {} bytes", nread),
///     None => println!("At the end of the stream!")
/// }
/// ```
pub struct PortReader {
    priv buf: Option<~[u8]>,  // A buffer of bytes received but not consumed.
    priv pos: uint,           // How many of the buffered bytes have already be consumed.
    priv port: Port<~[u8]>,   // The port to pull data from.
    priv closed: bool,        // Whether the pipe this port connects to has been closed.
}

impl PortReader {
    pub fn new(port: Port<~[u8]>) -> PortReader {
        PortReader {
            buf: None,
            pos: 0,
            port: port,
            closed: false,
        }
    }
}

impl Reader for PortReader {
    fn read(&mut self, buf: &mut [u8]) -> Option<uint> {
        let mut num_read = 0;
        loop {
            match self.buf {
                Some(ref prev) => {
                    let dst = buf.mut_slice_from(num_read);
                    let src = prev.slice_from(self.pos);
                    let count = cmp::min(dst.len(), src.len());
                    bytes::copy_memory(dst, src.slice_to(count));
                    num_read += count;
                    self.pos += count;
                },
                None => (),
            };
            if num_read == buf.len() || self.closed {
                break;
            }
            self.pos = 0;
            self.buf = self.port.recv_opt();
            self.closed = self.buf.is_none();
        }
        if self.closed && num_read == 0 {
            io::io_error::cond.raise(io::standard_error(io::EndOfFile));
            None
        } else {
            Some(num_read)
        }
    }
}

/// Allows writing to a chan.
///
/// # Example
///
/// ```
/// let writer = ChanWriter::new(chan);
/// writer.write("hello, world".as_bytes());
/// ```
pub struct ChanWriter {
    chan: Chan<~[u8]>,
}

impl ChanWriter {
    pub fn new(chan: Chan<~[u8]>) -> ChanWriter {
        ChanWriter { chan: chan }
    }
}

impl Writer for ChanWriter {
    fn write(&mut self, buf: &[u8]) {
        if !self.chan.try_send(buf.to_owned()) {
            io::io_error::cond.raise(io::IoError {
                kind: io::BrokenPipe,
                desc: "Pipe closed",
                detail: None
            });
        }
    }
}


#[cfg(test)]
mod test {
    use prelude::*;
    use super::*;
    use io;
    use task;

    #[test]
    fn test_port_reader() {
        let (port, chan) = Chan::new();
        do task::spawn {
          chan.send(~[1u8, 2u8]);
          chan.send(~[]);
          chan.send(~[3u8, 4u8]);
          chan.send(~[5u8, 6u8]);
          chan.send(~[7u8, 8u8]);
        }

        let mut reader = PortReader::new(port);
        let mut buf = ~[0u8, ..3];


        assert_eq!(Some(0), reader.read([]));

        assert_eq!(Some(3), reader.read(buf));
        assert_eq!(~[1,2,3], buf);

        assert_eq!(Some(3), reader.read(buf));
        assert_eq!(~[4,5,6], buf);

        assert_eq!(Some(2), reader.read(buf));
        assert_eq!(~[7,8,6], buf);

        let mut err = None;
        let result = io::io_error::cond.trap(|io::standard_error(k, _, _)| {
            err = Some(k)
        }).inside(|| {
            reader.read(buf)
        });
        assert_eq!(Some(io::EndOfFile), err);
        assert_eq!(None, result);
        assert_eq!(~[7,8,6], buf);

        // Ensure it continues to fail in the same way.
        err = None;
        let result = io::io_error::cond.trap(|io::standard_error(k, _, _)| {
            err = Some(k)
        }).inside(|| {
            reader.read(buf)
        });
        assert_eq!(Some(io::EndOfFile), err);
        assert_eq!(None, result);
        assert_eq!(~[7,8,6], buf);
    }

    #[test]
    fn test_chan_writer() {
        let (port, chan) = Chan::new();
        let mut writer = ChanWriter::new(chan);
        writer.write_be_u32(42);

        let wanted = ~[0u8, 0u8, 0u8, 42u8];
        let got = do task::try { port.recv() }.unwrap();
        assert_eq!(wanted, got);

        let mut err = None;
        io::io_error::cond.trap(|io::IoError { kind, .. } | {
            err = Some(kind)
        }).inside(|| {
            writer.write_u8(1)
        });
        assert_eq!(Some(io::BrokenPipe), err);
    }
}
