use crate::{ImageError, Result};

pub(crate) struct HeaderParser<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> HeaderParser<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    pub(crate) fn position(&self) -> usize {
        self.position
    }

    pub(crate) fn next_token(&mut self) -> Result<&'a [u8]> {
        self.skip_whitespace_and_comments();

        if self.position >= self.data.len() {
            return Err(ImageError::InvalidHeader {
                reason: "unexpected end of header",
            });
        }

        let start = self.position;

        while self.position < self.data.len() && !is_whitespace(self.data[self.position]) {
            self.position += 1;
        }

        Ok(&self.data[start..self.position])
    }

    pub(crate) fn next_u32(&mut self, name: &'static str) -> Result<u32> {
        let token = self.next_token()?;
        let text = std::str::from_utf8(token).map_err(|_| ImageError::InvalidHeader {
            reason: "header contains non-UTF-8 token",
        })?;

        text.parse::<u32>()
            .map_err(|_| ImageError::InvalidHeader { reason: name })
    }

    pub(crate) fn consume_raster_separator(&mut self) -> Result<()> {
        if self.position >= self.data.len() || !is_whitespace(self.data[self.position]) {
            return Err(ImageError::InvalidHeader {
                reason: "missing raster separator",
            });
        }

        if self.data[self.position] == b'\r'
            && self.position + 1 < self.data.len()
            && self.data[self.position + 1] == b'\n'
        {
            self.position += 2;
            return Ok(());
        }

        self.position += 1;
        Ok(())
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            while self.position < self.data.len() && is_whitespace(self.data[self.position]) {
                self.position += 1;
            }

            if self.position >= self.data.len() || self.data[self.position] != b'#' {
                break;
            }

            while self.position < self.data.len() && self.data[self.position] != b'\n' {
                self.position += 1;
            }
        }
    }
}

fn is_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_token_skips_whitespace_and_comments() {
        let mut parser = HeaderParser::new(b" \n# comment\nP6 2");

        assert_eq!(parser.next_token().unwrap(), b"P6");
        assert_eq!(parser.next_token().unwrap(), b"2");
    }

    #[test]
    fn next_u32_parses_integer_token() {
        let mut parser = HeaderParser::new(b"255");

        assert_eq!(parser.next_u32("max value").unwrap(), 255);
    }

    #[test]
    fn next_u32_rejects_invalid_integer_token() {
        let mut parser = HeaderParser::new(b"abc");
        let error = parser.next_u32("width").unwrap_err();

        assert_eq!(error, ImageError::InvalidHeader { reason: "width" });
    }

    #[test]
    fn consume_raster_separator_consumes_single_whitespace() {
        let mut parser = HeaderParser::new(b"255\nabc");

        assert_eq!(parser.next_token().unwrap(), b"255");
        parser.consume_raster_separator().unwrap();
        assert_eq!(parser.position(), 4);
    }

    #[test]
    fn consume_raster_separator_consumes_crlf_as_one_separator() {
        let mut parser = HeaderParser::new(b"255\r\nabc");

        assert_eq!(parser.next_token().unwrap(), b"255");
        parser.consume_raster_separator().unwrap();
        assert_eq!(parser.position(), 5);
    }
}
