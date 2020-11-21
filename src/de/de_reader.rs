use std::{collections::VecDeque, io::BufRead};

use crate::{events::Event, DeError, Reader};

// TODO: does it make sense to merge this with `Reader`?
/// Retrieve XML events from an underlying reader.
pub trait DeserializerReader<R: BufRead> {
    fn reader(&mut self) -> &mut Reader<R>;

    /// Get and "consume" the next event.
    fn next<'buf>(&mut self, buf: &'buf mut Vec<u8>) -> Result<Event<'static>, DeError>;

    /// Get the next event without consuming.
    fn peek(&mut self) -> Result<&Event<'static>, DeError>;

    /// Spawn a child buffer whose cursor starts at the same position as this buffer.
    fn child<'a>(&'a mut self) -> ChildDeserializerReader<'a, R>;
}

pub struct RootDeserializerReader<R: BufRead> {
    /// Underlying reader
    reader: Reader<R>,

    /// Cache of parsed but unused XML events, for peeking and out-of-order consumption
    evt_buffer: VecDeque<CachedEvent>,
    // TODO: add ephemeral buffer to be re-used when reading to avoid allocations, instead of
    // passing buf vecs into `.next()`
}

impl<R: BufRead> RootDeserializerReader<R> {
    pub fn new(reader: Reader<R>) -> Self {
        RootDeserializerReader {
            reader,
            evt_buffer: VecDeque::new(),
        }
    }
}

impl<R: BufRead> DeserializerReader<R> for RootDeserializerReader<R> {
    fn reader(&mut self) -> &mut Reader<R> {
        &mut self.reader
    }

    /// Consumed XML events in a root reader are removed from the start of the buffer
    fn next<'buf>(&mut self, buf: &'buf mut Vec<u8>) -> Result<Event<'static>, DeError> {
        loop {
            match self.evt_buffer.pop_front() {
                Some(CachedEvent::Unused(ev)) => break Ok(ev),
                Some(CachedEvent::Used) => continue,
                None => break next_significant_event(&mut self.reader, buf),
            }
        }
    }

    fn peek(&mut self) -> Result<&Event<'static>, DeError> {
        get_from_buffer_or_reader(&mut self.evt_buffer, &mut self.reader, &mut 0)
    }

    fn child<'root>(&'root mut self) -> ChildDeserializerReader<'root, R> {
        let RootDeserializerReader { reader, evt_buffer } = self;
        ChildDeserializerReader {
            reader,
            evt_buffer,
            evt_cursor: 0,
        }
    }
}

pub struct ChildDeserializerReader<'parent, R: BufRead> {
    reader: &'parent mut Reader<R>,
    evt_buffer: &'parent mut VecDeque<CachedEvent>,

    /// Position of this child deserializer in the parent buffer
    evt_cursor: usize,
}

impl<'parent, R: BufRead> ChildDeserializerReader<'parent, R> {
    /// Advance the child buffer without marking an event as "used"
    pub fn skip(&mut self) {
        self.evt_cursor += 1;
    }
}

impl<'parent, R: BufRead> DeserializerReader<R> for ChildDeserializerReader<'parent, R> {
    fn reader(&mut self) -> &mut Reader<R> {
        &mut self.reader
    }

    /// Consumed XML events in a child buffer are marked as "used" at their position in the buffer
    fn next<'buf>(&mut self, buf: &'buf mut Vec<u8>) -> Result<Event<'static>, DeError> {
        loop {
            match self.evt_buffer.get_mut(self.evt_cursor) {
                Some(entry @ CachedEvent::Unused(_)) => {
                    let taken = std::mem::replace(entry, CachedEvent::Used);

                    match taken {
                        CachedEvent::Unused(ev) => return Ok(ev),
                        _ => panic!("CacheEvent buffer index should not be empty"),
                    }
                }
                Some(CachedEvent::Used) => {
                    self.evt_cursor += 1;
                    continue;
                }
                None => {
                    debug_assert_eq!(self.evt_buffer.len(), self.evt_cursor);

                    // Skip creation of buffer entry when consuming event straight away
                    return next_significant_event(&mut self.reader, buf);
                }
            }
        }
    }

    fn peek(&mut self) -> Result<&Event<'static>, DeError> {
        get_from_buffer_or_reader(self.evt_buffer, self.reader, &mut self.evt_cursor)
    }

    fn child<'a>(&'a mut self) -> ChildDeserializerReader<'a, R> {
        let ChildDeserializerReader {
            reader,
            evt_buffer: buffer,
            evt_cursor: cursor,
        } = self;

        ChildDeserializerReader {
            reader,
            evt_buffer: buffer,
            evt_cursor: *cursor,
        }
    }
}

#[derive(Debug)]
enum CachedEvent {
    Unused(Event<'static>),
    Used,
}

fn get_from_buffer_or_reader<'ebuf>(
    evt_buf: &'ebuf mut VecDeque<CachedEvent>,
    reader: &mut Reader<impl BufRead>,
    index: &mut usize,
) -> Result<&'ebuf Event<'static>, DeError> {
    let mut xml_bytes_buf = Vec::new();
    loop {
        match evt_buf.get_mut(*index) {
            Some(CachedEvent::Unused(_)) => break,
            Some(CachedEvent::Used) => {
                *index += 1;
            }
            None => {
                let next = next_significant_event(reader, &mut xml_bytes_buf)?;
                evt_buf.push_back(CachedEvent::Unused(next));
            }
        }
    }

    // Returning of borrowed data must be done outside of loop/match due to current limitation of borrow checker
    match evt_buf.get_mut(*index) {
        Some(CachedEvent::Unused(event)) => Ok(event),
        _ => panic!("CachedEvent buffer index should not be empty"),
    }
}

/// Reads the next XML event from the underlying reader, skipping events we're not interested in.
fn next_significant_event<'buf>(
    reader: &mut Reader<impl BufRead>,
    buf: &'buf mut Vec<u8>,
) -> Result<Event<'static>, DeError> {
    loop {
        let e = reader.read_event(buf)?;
        match e {
            Event::Start(_) | Event::End(_) | Event::Text(_) | Event::Eof | Event::CData(_) => {
                return Ok(e.into_owned())
            }
            _ => buf.clear(),
        }
    }
}
