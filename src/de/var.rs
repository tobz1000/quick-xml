use crate::{
    de::{de_reader::DeserializerReader, escape::EscapedDeserializer, Deserializer},
    errors::serialize::DeError,
    events::Event,
};
use serde::de::{self, Deserializer as SerdeDeserializer};
use std::io::BufRead;

/// An enum access
pub struct EnumAccess<'a, R: BufRead, D: DeserializerReader<R>> {
    de: &'a mut Deserializer<R, D>,
}

impl<'a, R: BufRead, D: DeserializerReader<R>> EnumAccess<'a, R, D> {
    pub fn new(de: &'a mut Deserializer<R, D>) -> Self {
        EnumAccess { de }
    }
}

impl<'de, 'a, R: 'a + BufRead, D: DeserializerReader<R>> de::EnumAccess<'de>
    for EnumAccess<'a, R, D>
{
    type Error = DeError;
    type Variant = VariantAccess<'a, R, D>;

    fn variant_seed<V: de::DeserializeSeed<'de>>(
        self,
        seed: V,
    ) -> Result<(V::Value, VariantAccess<'a, R, D>), DeError> {
        let decoder = self.de.de_reader.reader().decoder();
        let de = match self.de.peek()? {
            Event::Text(t) => EscapedDeserializer::new(t.to_vec(), decoder, true),
            Event::Start(e) => EscapedDeserializer::new(e.name().to_vec(), decoder, false),
            e => return Err(DeError::InvalidEnum(e.to_owned())),
        };
        let name = seed.deserialize(de)?;
        Ok((name, VariantAccess { de: self.de }))
    }
}

pub struct VariantAccess<'a, R: BufRead, D: DeserializerReader<R>> {
    de: &'a mut Deserializer<R, D>,
}

impl<'de, 'a, R: BufRead, D: DeserializerReader<R>> de::VariantAccess<'de>
    for VariantAccess<'a, R, D>
{
    type Error = DeError;

    fn unit_variant(self) -> Result<(), DeError> {
        match self.de.next(&mut Vec::new())? {
            Event::Start(e) => self.de.read_to_end(e.name()),
            Event::Text(_) => Ok(()),
            _ => unreachable!(),
        }
    }

    fn newtype_variant_seed<T: de::DeserializeSeed<'de>>(
        self,
        seed: T,
    ) -> Result<T::Value, DeError> {
        seed.deserialize(&mut *self.de)
    }

    fn tuple_variant<V: de::Visitor<'de>>(
        self,
        len: usize,
        visitor: V,
    ) -> Result<V::Value, DeError> {
        self.de.deserialize_tuple(len, visitor)
    }

    fn struct_variant<V: de::Visitor<'de>>(
        self,
        fields: &'static [&'static str],
        visitor: V,
    ) -> Result<V::Value, DeError> {
        self.de.deserialize_struct("", fields, visitor)
    }
}
