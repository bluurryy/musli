use musli::de::VariantDecoder;
use musli::Context;

use crate::reader::{Parser, Token};

use super::{JsonDecoder, JsonKeyDecoder};

pub(crate) struct JsonVariantDecoder<P> {
    parser: P,
}

impl<'de, P> JsonVariantDecoder<P>
where
    P: Parser<'de>,
{
    #[inline]
    pub(crate) fn new<C>(cx: &C, mut parser: P) -> Result<Self, C::Error>
    where
        C: ?Sized + Context,
    {
        parser.skip_whitespace(cx)?;

        let actual = parser.peek(cx)?;

        if !matches!(actual, Token::OpenBrace) {
            return Err(cx.message(format_args!("Expected open brace, was {actual}")));
        }

        parser.skip(cx, 1)?;
        Ok(Self { parser })
    }
}

impl<'de, C, P> VariantDecoder<'de, C> for JsonVariantDecoder<P>
where
    C: ?Sized + Context,
    P: Parser<'de>,
{
    type DecodeTag<'this> = JsonKeyDecoder<P::Mut<'this>>
    where
        Self: 'this;
    type DecodeVariant<'this> = JsonDecoder<P::Mut<'this>> where Self: 'this;

    #[inline]
    fn decode_tag(&mut self, _: &C) -> Result<Self::DecodeTag<'_>, C::Error> {
        Ok(JsonKeyDecoder::new(self.parser.borrow_mut()))
    }

    #[inline]
    fn decode_value(&mut self, cx: &C) -> Result<Self::DecodeVariant<'_>, C::Error> {
        let actual = self.parser.peek(cx)?;

        if !matches!(actual, Token::Colon) {
            return Err(cx.message(format_args!("Expected colon, was {actual}")));
        }

        self.parser.skip(cx, 1)?;
        Ok(JsonDecoder::new(self.parser.borrow_mut()))
    }

    #[inline]
    fn skip_value(&mut self, cx: &C) -> Result<bool, C::Error> {
        let this = self.decode_value(cx)?;
        JsonDecoder::new(this.parser).skip_any(cx)?;
        Ok(true)
    }

    #[inline]
    fn end(mut self, cx: &C) -> Result<(), C::Error> {
        let actual = self.parser.peek(cx)?;

        if !matches!(actual, Token::CloseBrace) {
            return Err(cx.message(format_args!("Expected closing brace, was {actual}")));
        }

        self.parser.skip(cx, 1)?;
        Ok(())
    }
}
