use super::{ParseError, ParseResult};
use crate::Parser;
use trunk_lexer::TokenKind;

impl Parser {
    /// Expect an unqualified identifier such as Foo or Bar.
    pub(crate) fn ident(&mut self) -> ParseResult<Vec<u8>> {
        Ok(expect!(
            self,
            TokenKind::Identifier(i),
            i,
            "expected identifier"
        ))
    }

    /// Expect an unqualified or qualified identifier such as Foo, Bar or Foo\Bar.
    pub(crate) fn name(&mut self) -> ParseResult<Vec<u8>> {
        Ok(expect!(
            self,
            TokenKind::Identifier(i) | TokenKind::QualifiedIdentifier(i),
            i,
            "expected identifier"
        ))
    }

    /// Expect an unqualified, qualified or fully qualified identifier such as Foo, Foo\Bar or \Foo\Bar.
    pub(crate) fn full_name(&mut self) -> ParseResult<Vec<u8>> {
        Ok(expect!(
            self,
            TokenKind::Identifier(i)
                | TokenKind::QualifiedIdentifier(i)
                | TokenKind::FullyQualifiedIdentifier(i),
            i,
            "expected identifier"
        ))
    }

    pub(crate) fn var(&mut self) -> ParseResult<Vec<u8>> {
        Ok(expect!(
            self,
            TokenKind::Variable(v),
            v,
            "expected variable name"
        ))
    }

    pub(crate) fn full_name_maybe_type_keyword(&mut self) -> ParseResult<Vec<u8>> {
        match self.current.kind {
            TokenKind::Array | TokenKind::Callable => {
                let r = Ok(self.current.kind.to_string().into());
                self.next();
                r
            }
            _ => self.full_name(),
        }
    }

    pub(crate) fn type_with_static(&mut self) -> ParseResult<Vec<u8>> {
        Ok(match self.current.kind {
            TokenKind::Static => {
                self.next();
                "static".into()
            }
            _ => self.full_name_maybe_type_keyword()?,
        })
    }

    pub(crate) fn ident_maybe_reserved(&mut self) -> ParseResult<Vec<u8>> {
        match self.current.kind {
            TokenKind::Static
            | TokenKind::Abstract
            | TokenKind::Final
            | TokenKind::For
            | TokenKind::Private
            | TokenKind::Protected
            | TokenKind::Public
            | TokenKind::Require
            | TokenKind::RequireOnce
            | TokenKind::New
            | TokenKind::Clone
            | TokenKind::If
            | TokenKind::Else
            | TokenKind::ElseIf
            | TokenKind::Default
            | TokenKind::Enum
            | TokenKind::Match
            | TokenKind::Catch
            | TokenKind::Finally
            | TokenKind::Namespace => {
                let string = self.current.kind.to_string().into();
                self.next();
                Ok(string)
            }
            _ => self.ident(),
        }
    }
}
