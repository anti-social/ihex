use std::io::{BufRead, Read};

use crate::{Reader, Record};

#[derive(Clone, Copy, Debug)]
pub enum StartAddress {
    Segment {
        cs: u16,
        ip: u16,
    },
    Linear(u32),
}

pub struct BinaryReader<R: BufRead> {
    reader: Reader<R>,
    record: Option<Record>,
    record_pos: usize,
    base_address: u32,
    address: u32,
    read_bytes: u32,
    start_address: Option<StartAddress>,
}

impl<R: BufRead> BinaryReader<R> {
    pub fn new(reader: Reader<R>) -> Self {
        Self {
            reader,
            record: None,
            record_pos: 0,
            base_address: 0,
            address: 0,
            read_bytes: 0,
            start_address: None,
        }
    }

    pub fn start_address(&self) -> Option<StartAddress> {
        return self.start_address;
    }

    fn update_address(&mut self, addr: u32) {
        if self.base_address == 0 && self.read_bytes == 0 {
            self.base_address = addr;
        } else {
            self.address = addr - self.base_address;
        }
    }
}

impl<R: BufRead> Read for BinaryReader<R> {
    fn read(&mut self, mut buf: &mut [u8]) -> Result<usize, std::io::Error> {
        let buf_len = buf.len();
        loop {
            if let Some(ref record) = self.record {
                match record {
                    Record::ExtendedSegmentAddress(addr) => {
                        self.update_address((*addr as u32) << 4);
                        self.record = None;
                    }
                    Record::ExtendedLinearAddress(addr) => {
                        self.update_address((*addr as u32) << 16);
                        self.record = None;
                    }
                    Record::Data { offset, value } => {
                        let value = &value[self.record_pos..];
                        let record_address = self.address + *offset as u32;
                        if record_address > self.read_bytes {
                            let zero_bytes_left = (record_address - self.read_bytes) as usize;
                            let zeroed_len = {
                                let buf_to_fill = if zero_bytes_left >= buf.len() {
                                    &mut buf[..]
                                } else {
                                    &mut buf[..zero_bytes_left]
                                };
                                buf_to_fill.fill(0);
                                buf_to_fill.len()
                            };
                            buf = &mut buf[zeroed_len..];
                            self.read_bytes += zeroed_len as u32;
                        }

                        if buf.len() == 0 {
                            return Ok(buf_len);
                        }

                        if value.len() >= buf.len() {
                            buf.copy_from_slice(&value[..buf.len()]);
                            self.record_pos += buf.len();
                            self.read_bytes += buf.len() as u32;
                            return Ok(buf_len);
                        }

                        buf[..value.len()].copy_from_slice(value);
                        buf = &mut buf[value.len()..];
                        self.read_bytes += value.len() as u32;
                        self.record = None;
                        self.record_pos = 0;
                    }
                    Record::StartSegmentAddress { cs, ip } => {
                        self.start_address = Some(
                            StartAddress::Segment { cs: *cs, ip: *ip }
                        );
                        self.record = None;
                    }
                    Record::StartLinearAddress(addr) => {
                        self.start_address = Some(StartAddress::Linear(*addr));
                        self.record = None;
                    }
                    Record::EndOfFile => {
                        return Ok(buf_len - buf.len());
                    }
                }
            }

            if self.record.is_none() {
                if let Some(record) = self.reader.next() {
                    match record {
                        Ok(record) => {
                            self.record = Some(record);
                        }
                        Err(e) => return Err(
                            std::io::Error::other(
                                format!("Error reading hex file: {e}")
                            )
                        ),
                    }
                } else {
                    return Ok(buf_len - buf.len());
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Read};

    use assert_matches::assert_matches;
    use indoc::indoc;

    use crate::Reader;
    use super::BinaryReader;

    #[test]
    fn test_binary_reader() {
        let input = indoc! {"
            :020000040800F2
            :10000000F0FF001029BA05086D440508DDE6010877
        "};

        let reader = Reader::new(Cursor::new(input));
        let mut binary_reader = BinaryReader::new(reader);
        let mut buf = [0; 12];

        assert_matches!(
            binary_reader.read(&mut buf),
            Ok(n) => {
                assert_eq!(n, 12);
                assert_eq!(
                    &buf[..],
                    &[0xf0, 0xff, 0x00, 0x10, 0x29, 0xba, 0x05, 0x08, 0x6d, 0x44, 0x05, 0x08]
                );
            }
        );

        assert_matches!(
            binary_reader.read(&mut buf),
            Ok(n) => {
                assert_eq!(n, 4);
                assert_eq!(
                    &buf[..n],
                    &[0xdd, 0xe6, 0x01, 0x08]
                );
            }
        );
    }

    #[test]
    fn test_binary_reader_skip() {
        let input = indoc! {"
            :020000040800F2
            :02000000F0FF0F
            :020000040801F1
            :02000000F0FF0F
        "};

        let reader = Reader::new(Cursor::new(input));
        let mut binary_reader = BinaryReader::new(reader);
        let mut buf = [0; 12];

        assert_matches!(
            binary_reader.read(&mut buf),
            Ok(n) => {
                assert_eq!(n, 12);
                assert_eq!(
                    &buf[..],
                    &[0xf0, 0xff, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0]
                );
            }
        );

        let mut huge_buf = [0; 65536 - 10];
        assert_matches!(
            binary_reader.read(&mut huge_buf),
            Ok(n) => {
                assert_eq!(n, huge_buf.len());
                assert_eq!(
                    &huge_buf[..8],
                    &[0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0x0]
                );
                assert_eq!(
                    &huge_buf[huge_buf.len() - 8..],
                    &[0x0, 0x0, 0x0, 0x0, 0x0, 0x0, 0xf0, 0xff]
                );
            }
        );
    }
}
