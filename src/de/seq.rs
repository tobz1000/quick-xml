use crate::de::{de_reader::DeserializerReader, ChildDeserializer, DeError};
use crate::events::Event;
use serde::de;
use std::io::BufRead;

#[derive(Debug)]
enum Names {
    Unknown,
    Peek(String),
}

/// A SeqAccess
pub struct SeqAccess<'a, R: BufRead> {
    de: ChildDeserializer<'a, R>,
    max_size: Option<usize>,
    names: Names,
}

impl<'a, R: BufRead> SeqAccess<'a, R> {
    /// Get a new SeqAccess
    pub fn new(mut de: ChildDeserializer<'a, R>, max_size: Option<usize>) -> Result<Self, DeError> {
        let decoder = de.de_reader.reader().decoder();
        let names = if de.has_value_field {
            Names::Unknown
        } else {
            if let Event::Start(e) = de.peek()? {
                #[cfg(not(feature = "encoding"))]
                let name = decoder.decode(e.name())?.to_owned();
                #[cfg(feature = "encoding")]
                let name = decoder.decode(e.name()).into_owned();
                Names::Peek(name)
            } else {
                Names::Unknown
            }
        };
        Ok(SeqAccess {
            de,
            max_size,
            names,
        })
    }
}

impl<'de, 'a, R: 'a + BufRead> de::SeqAccess<'de> for SeqAccess<'a, R> {
    type Error = DeError;

    fn size_hint(&self) -> Option<usize> {
        self.max_size
    }

    fn next_element_seed<T: de::DeserializeSeed<'de>>(
        &mut self,
        seed: T,
    ) -> Result<Option<T::Value>, DeError> {
        if let Some(s) = self.max_size.as_mut() {
            if *s == 0 {
                return Ok(None);
            }
            *s -= 1;
        }
        let decoder = self.de.de_reader.reader().decoder();
        match &self.names {
            Names::Peek(expected_name) => {
                let mut local_depth = 0;
                loop {
                    let next_element = self.de.peek()?;
                    match next_element {
                        Event::Start(start) => {
                            #[cfg(not(feature = "encoding"))]
                            let name = decoder.decode(start.name())?;
                            #[cfg(feature = "encoding")]
                            let name = decoder.decode(start.name());

                            if name == expected_name && local_depth == 0 {
                                return seed.deserialize(&mut self.de).map(Some);
                            } else {
                                local_depth += 1;
                                self.de.de_reader.skip();
                            }
                        }
                        Event::End(_) => {
                            if local_depth == 0 {
                                return Ok(None);
                            } else {
                                local_depth -= 1;
                                self.de.de_reader.skip();
                            }
                        }
                        Event::Eof => {
                            return Ok(None);
                        }
                        _ => {
                            self.de.de_reader.skip();
                        }
                    }
                }
            }
            Names::Unknown => {
                let next_element = self.de.peek()?;

                match next_element {
                    Event::Eof | Event::End(_) => {
                        return Ok(None);
                    }
                    _ => {
                        return seed.deserialize(&mut self.de).map(Some);
                    }
                }
            }
        }
    }
}
