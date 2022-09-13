use trunk_lexer::TokenKind;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Precedence {
    Lowest,
    CloneNew,
    Pow,
    Prefix,
    Instanceof,
    Bang,
    MulDivMod,
    AddSub,
    BitShift,
    Concat,
    LtGt,
    Equality,
    BitwiseAnd,
    BitwiseXor,
    BitwiseOr,
    And,
    Or,
    NullCoalesce,
    Ternary,
    Assignment,
    YieldFrom,
    Yield,
    Print,
    KeyAnd,
    KeyXor,
    KeyOr,
    CallDim,
    ObjectAccess,
    IncDec,
}

impl Precedence {
    pub fn prefix(kind: &TokenKind) -> Self {
        use TokenKind::*;

        match kind {
            Bang => Self::Bang,
            Clone | New => Self::CloneNew,
            _ => Self::Prefix,
        }
    }

    pub fn infix(kind: &TokenKind) -> Self {
        use TokenKind::*;

        match kind {
            Pow => Self::Pow,
            Instanceof => Self::Instanceof,
            Asterisk | Slash | Percent => Self::MulDivMod,
            Plus | Minus => Self::AddSub,
            LeftShift | RightShift => Self::BitShift,
            Dot => Self::Concat,
            LessThan | LessThanEquals | GreaterThan | GreaterThanEquals => Self::LtGt,
            DoubleEquals | BangEquals | TripleEquals | BangDoubleEquals => Self::Equality,
            Ampersand => Self::BitwiseAnd,
            Caret => Self::BitwiseXor,
            Pipe => Self::BitwiseOr,
            BooleanAnd => Self::And,
            BooleanOr => Self::Or,
            Coalesce => Self::NullCoalesce,
            Question => Self::Ternary,
            Equals | PlusEquals | MinusEquals | AsteriskEqual | PowEquals | SlashEquals
            | DotEquals | AndEqual | CoalesceEqual => Self::Assignment,
            Yield => Self::Yield,
            _ => unimplemented!("precedence for op {:?}", kind),
        }
    }

    pub fn postfix(kind: &TokenKind) -> Self {
        use TokenKind::*;

        match kind {
            Coalesce => Self::NullCoalesce,
            Increment | Decrement => Self::IncDec,
            LeftParen | LeftBracket => Self::CallDim,
            Arrow | NullsafeArrow | DoubleColon => Self::ObjectAccess,
            _ => unimplemented!("postfix precedence for op {:?}", kind),
        }
    }

    pub fn associativity(&self) -> Option<Associativity> {
        Some(match self {
            Self::Instanceof
            | Self::MulDivMod
            | Self::AddSub
            | Self::BitShift
            | Self::Concat
            | Self::BitwiseAnd
            | Self::BitwiseOr
            | Self::BitwiseXor
            | Self::And
            | Self::Or
            | Self::KeyAnd
            | Self::KeyOr
            | Self::KeyXor => Associativity::Left,
            Self::Pow | Self::NullCoalesce | Self::Assignment => Associativity::Right,
            Self::Ternary | Self::Equality | Self::LtGt => Associativity::Non,
            _ => return None,
        })
    }
}

pub enum Associativity {
    Non,
    Left,
    Right,
}
