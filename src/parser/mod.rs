use crate::expect_literal;
use crate::expect_token;
use crate::expected_token_err;
use crate::lexer::token::Token;
use crate::lexer::token::TokenKind;
use crate::lexer::DocStringKind;
use crate::parser::ast::comments::Comment;
use crate::parser::ast::comments::CommentFormat;
use crate::parser::ast::identifiers::Identifier;
use crate::parser::ast::variables::Variable;
use crate::parser::ast::DefaultMatchArm;
use crate::parser::ast::{
    ArrayItem, Constant, DeclareItem, Expression, IncludeKind, MagicConst, MatchArm, Program,
    Statement, StaticVar, StringPart, Use, UseKind,
};
use crate::parser::error::ParseError;
use crate::parser::error::ParseResult;
use crate::parser::internal::identifiers;
use crate::parser::internal::identifiers::is_reserved_ident;
use crate::parser::internal::precedences::{Associativity, Precedence};
use crate::parser::internal::utils;
use crate::parser::state::State;

pub mod ast;
pub mod error;

mod internal;
mod macros;
mod state;

#[derive(Debug, PartialEq, Eq, Clone, Copy, Default)]
pub struct Parser;

impl Parser {
    pub const fn new() -> Self {
        Self {}
    }

    pub fn parse(&self, tokens: Vec<Token>) -> ParseResult<Program> {
        let mut state = State::new(tokens);

        let mut ast = Program::new();

        while state.current.kind != TokenKind::Eof {
            if matches!(
                state.current.kind,
                TokenKind::OpenTag(_) | TokenKind::CloseTag
            ) {
                state.next();
                continue;
            }

            state.gather_comments();

            if state.is_eof() {
                break;
            }

            ast.push(self.top_level_statement(&mut state)?);

            state.clear_comments();
        }

        Ok(ast.to_vec())
    }

    fn top_level_statement(&self, state: &mut State) -> ParseResult<Statement> {
        state.skip_comments();

        let statement = match &state.current.kind {
            TokenKind::Namespace => self.namespace(state)?,
            TokenKind::Use => {
                state.next();

                let kind = match state.current.kind {
                    TokenKind::Function => {
                        state.next();
                        UseKind::Function
                    }
                    TokenKind::Const => {
                        state.next();
                        UseKind::Const
                    }
                    _ => UseKind::Normal,
                };

                if state.peek.kind == TokenKind::LeftBrace {
                    let prefix = identifiers::full_name(state)?;
                    state.next();

                    let mut uses = Vec::new();
                    while state.current.kind != TokenKind::RightBrace {
                        let name = identifiers::full_name(state)?;
                        let mut alias = None;

                        if state.current.kind == TokenKind::As {
                            state.next();
                            alias = Some(identifiers::ident(state)?);
                        }

                        uses.push(Use { name, alias });

                        if state.current.kind == TokenKind::Comma {
                            state.next();
                            continue;
                        }
                    }

                    utils::skip_right_brace(state)?;
                    utils::skip_semicolon(state)?;

                    Statement::GroupUse { prefix, kind, uses }
                } else {
                    let mut uses = Vec::new();
                    while !state.is_eof() {
                        let name = identifiers::full_name(state)?;
                        let mut alias = None;

                        if state.current.kind == TokenKind::As {
                            state.next();
                            alias = Some(identifiers::ident(state)?);
                        }

                        uses.push(Use { name, alias });

                        if state.current.kind == TokenKind::Comma {
                            state.next();
                            continue;
                        }

                        utils::skip_semicolon(state)?;
                        break;
                    }

                    Statement::Use { uses, kind }
                }
            }
            TokenKind::Const => {
                state.next();

                let mut constants = vec![];

                loop {
                    let name = identifiers::ident(state)?;

                    utils::skip(state, TokenKind::Equals)?;

                    let value = self.expression(state, Precedence::Lowest)?;

                    constants.push(Constant { name, value });

                    if state.current.kind == TokenKind::Comma {
                        state.next();
                    } else {
                        break;
                    }
                }

                utils::skip_semicolon(state)?;

                Statement::Constant { constants }
            }
            TokenKind::HaltCompiler => {
                state.next();

                let content = if let TokenKind::InlineHtml(content) = state.current.kind.clone() {
                    state.next();
                    Some(content)
                } else {
                    None
                };

                Statement::HaltCompiler { content }
            }
            _ => self.statement(state)?,
        };

        state.clear_comments();

        // A closing PHP tag is valid after the end of any top-level statement.
        if state.current.kind == TokenKind::CloseTag {
            state.next();
        }

        Ok(statement)
    }

    fn statement(&self, state: &mut State) -> ParseResult<Statement> {
        let has_attributes = self.gather_attributes(state)?;

        let statement = if has_attributes {
            match &state.current.kind {
                TokenKind::Abstract => self.class_definition(state)?,
                TokenKind::Readonly => self.class_definition(state)?,
                TokenKind::Final => self.class_definition(state)?,
                TokenKind::Class => self.class_definition(state)?,
                TokenKind::Interface => self.interface_definition(state)?,
                TokenKind::Trait => self.trait_definition(state)?,
                TokenKind::Enum => self.enum_definition(state)?,
                TokenKind::Function
                    if matches!(
                        state.peek.kind,
                        TokenKind::Identifier(_) | TokenKind::Null | TokenKind::Ampersand
                    ) =>
                {
                    // FIXME: This is incredibly hacky but we don't have a way to look at
                    // the next N tokens right now. We could probably do with a `peek_buf()`
                    // method like the Lexer has.
                    if state.peek.kind == TokenKind::Ampersand {
                        let mut cloned = state.iter.clone();
                        if let Some((index, _)) = state.iter.clone().enumerate().next() {
                            if !matches!(
                                cloned.nth(index),
                                Some(Token {
                                    kind: TokenKind::Identifier(_),
                                    ..
                                })
                            ) {
                                let expr = self.expression(state, Precedence::Lowest)?;

                                utils::skip_semicolon(state)?;

                                return Ok(Statement::Expression { expr });
                            }
                        }

                        self.function(state)?
                    } else {
                        self.function(state)?
                    }
                }
                _ => {
                    // Note, we can get attributes and know their span, maybe use that in the
                    // error in the future?
                    return Err(ParseError::ExpectedItemDefinitionAfterAttributes(
                        state.current.span,
                    ));
                }
            }
        } else {
            match &state.current.kind {
                TokenKind::Abstract => self.class_definition(state)?,
                TokenKind::Readonly => self.class_definition(state)?,
                TokenKind::Final => self.class_definition(state)?,
                TokenKind::Class => self.class_definition(state)?,
                TokenKind::Interface => self.interface_definition(state)?,
                TokenKind::Trait => self.trait_definition(state)?,
                TokenKind::Enum => self.enum_definition(state)?,
                TokenKind::Function
                    if matches!(
                        state.peek.kind,
                        TokenKind::Identifier(_) | TokenKind::Null | TokenKind::Ampersand
                    ) =>
                {
                    // FIXME: This is incredibly hacky but we don't have a way to look at
                    // the next N tokens right now. We could probably do with a `peek_buf()`
                    // method like the Lexer has.
                    if state.peek.kind == TokenKind::Ampersand {
                        if let Some((_, token)) = state.iter.clone().enumerate().next() {
                            if !matches!(
                                token,
                                Token {
                                    kind: TokenKind::Identifier(_),
                                    ..
                                }
                            ) {
                                let expr = self.expression(state, Precedence::Lowest)?;

                                utils::skip_semicolon(state)?;

                                return Ok(Statement::Expression { expr });
                            }
                        }

                        self.function(state)?
                    } else {
                        self.function(state)?
                    }
                }
                TokenKind::Goto => {
                    state.next();

                    let label = identifiers::ident(state)?;

                    utils::skip_semicolon(state)?;

                    Statement::Goto { label }
                }
                TokenKind::Identifier(_) if state.peek.kind == TokenKind::Colon => {
                    let label = identifiers::ident(state)?;

                    utils::colon(state)?;

                    Statement::Label { label }
                }
                TokenKind::Declare => {
                    state.next();
                    utils::skip_left_parenthesis(state)?;

                    let mut declares = Vec::new();
                    loop {
                        let key = identifiers::ident(state)?;

                        utils::skip(state, TokenKind::Equals)?;

                        let value = expect_literal!(state);

                        declares.push(DeclareItem { key, value });

                        if state.current.kind == TokenKind::Comma {
                            state.next();
                        } else {
                            break;
                        }
                    }

                    utils::skip_right_parenthesis(state)?;

                    let body = if state.current.kind == TokenKind::LeftBrace {
                        state.next();
                        let b = self.body(state, &TokenKind::RightBrace)?;
                        utils::skip_right_brace(state)?;
                        b
                    } else if state.current.kind == TokenKind::Colon {
                        utils::colon(state)?;
                        let b = self.body(state, &TokenKind::EndDeclare)?;
                        utils::skip(state, TokenKind::EndDeclare)?;
                        utils::skip_semicolon(state)?;
                        b
                    } else {
                        utils::skip_semicolon(state)?;
                        vec![]
                    };

                    Statement::Declare { declares, body }
                }
                TokenKind::Global => {
                    state.next();

                    let mut vars = vec![];
                    // `loop` instead of `while` as we don't allow for extra commas.
                    loop {
                        vars.push(identifiers::var(state)?);

                        if state.current.kind == TokenKind::Comma {
                            state.next();
                        } else {
                            break;
                        }
                    }

                    utils::skip_semicolon(state)?;
                    Statement::Global { vars }
                }
                TokenKind::Static if matches!(state.peek.kind, TokenKind::Variable(_)) => {
                    state.next();

                    let mut vars = vec![];

                    // `loop` instead of `while` as we don't allow for extra commas.
                    loop {
                        let var = identifiers::var(state)?;
                        let mut default = None;

                        if state.current.kind == TokenKind::Equals {
                            state.next();

                            default = Some(self.expression(state, Precedence::Lowest)?);
                        }

                        // TODO: group static vars.
                        vars.push(StaticVar { var, default });

                        if state.current.kind == TokenKind::Comma {
                            state.next();
                        } else {
                            break;
                        }
                    }

                    utils::skip_semicolon(state)?;

                    Statement::Static { vars }
                }
                TokenKind::InlineHtml(html) => {
                    let s = Statement::InlineHtml(html.clone());
                    state.next();
                    s
                }
                TokenKind::SingleLineComment(comment) => {
                    let start = state.current.span;
                    let content = comment.clone();
                    state.next();
                    let end = state.current.span;
                    let format = CommentFormat::SingleLine;

                    Statement::Comment(Comment {
                        start,
                        end,
                        format,
                        content,
                    })
                }
                TokenKind::MultiLineComment(comment) => {
                    let start = state.current.span;
                    let content = comment.clone();
                    state.next();
                    let end = state.current.span;
                    let format = CommentFormat::MultiLine;

                    Statement::Comment(Comment {
                        start,
                        end,
                        format,
                        content,
                    })
                }
                TokenKind::HashMarkComment(comment) => {
                    let start = state.current.span;
                    let content = comment.clone();
                    state.next();
                    let end = state.current.span;
                    let format = CommentFormat::HashMark;

                    Statement::Comment(Comment {
                        start,
                        end,
                        format,
                        content,
                    })
                }
                TokenKind::DocumentComment(comment) => {
                    let start = state.current.span;
                    let content = comment.clone();
                    state.next();
                    let end = state.current.span;
                    let format = CommentFormat::Document;

                    Statement::Comment(Comment {
                        start,
                        end,
                        format,
                        content,
                    })
                }
                TokenKind::Do => self.do_loop(state)?,
                TokenKind::While => self.while_loop(state)?,
                TokenKind::For => self.for_loop(state)?,
                TokenKind::Foreach => self.foreach_loop(state)?,
                TokenKind::Switch => self.switch_statement(state)?,
                TokenKind::Continue => self.continue_statement(state)?,
                TokenKind::Break => self.break_statement(state)?,
                TokenKind::If => self.if_statement(state)?,
                TokenKind::Echo => {
                    state.next();

                    let mut values = Vec::new();
                    loop {
                        values.push(self.expression(state, Precedence::Lowest)?);

                        if state.current.kind == TokenKind::Comma {
                            state.next();
                        } else {
                            break;
                        }
                    }

                    utils::skip_semicolon(state)?;
                    Statement::Echo { values }
                }
                TokenKind::Return => {
                    state.next();

                    if let Token {
                        kind: TokenKind::SemiColon,
                        ..
                    } = state.current
                    {
                        let ret = Statement::Return { value: None };
                        utils::skip_semicolon(state)?;
                        ret
                    } else {
                        let ret = Statement::Return {
                            value: self.expression(state, Precedence::Lowest).ok(),
                        };
                        utils::skip_semicolon(state)?;
                        ret
                    }
                }
                TokenKind::SemiColon => {
                    state.next();

                    Statement::Noop
                }
                TokenKind::Try => self.try_block(state)?,
                TokenKind::LeftBrace => self.block_statement(state)?,
                _ => {
                    let expr = self.expression(state, Precedence::Lowest)?;

                    utils::skip_semicolon(state)?;

                    Statement::Expression { expr }
                }
            }
        };

        state.skip_comments();

        // A closing PHP tag is valid after the end of any top-level statement.
        if state.current.kind == TokenKind::CloseTag {
            state.next();
        }

        Ok(statement)
    }

    fn expression(&self, state: &mut State, precedence: Precedence) -> ParseResult<Expression> {
        if state.is_eof() {
            return Err(ParseError::UnexpectedEndOfFile);
        }

        let has_attributes = self.gather_attributes(state)?;

        let mut left = if has_attributes {
            match &state.current.kind {
                TokenKind::Static if state.peek.kind == TokenKind::Function => {
                    self.anonymous_function(state)?
                }
                TokenKind::Static if state.peek.kind == TokenKind::Fn => {
                    self.arrow_function(state)?
                }
                TokenKind::Function => self.anonymous_function(state)?,
                TokenKind::Fn => self.arrow_function(state)?,
                _ => {
                    // Note, we can get attributes and know their span, maybe use that in the
                    // error in the future?
                    return Err(ParseError::ExpectedItemDefinitionAfterAttributes(
                        state.current.span,
                    ));
                }
            }
        } else {
            match &state.current.kind {
                TokenKind::List => self.list_expression(state)?,
                TokenKind::Static if state.peek.kind == TokenKind::Function => {
                    self.anonymous_function(state)?
                }
                TokenKind::Static if state.peek.kind == TokenKind::Fn => {
                    self.arrow_function(state)?
                }
                TokenKind::Function => self.anonymous_function(state)?,
                TokenKind::Fn => self.arrow_function(state)?,
                TokenKind::New
                    if state.peek.kind == TokenKind::Class
                        || state.peek.kind == TokenKind::Attribute =>
                {
                    self.anonymous_class_definition(state)?
                }
                TokenKind::Throw => {
                    state.next();

                    let value = self.expression(state, Precedence::Lowest)?;

                    Expression::Throw {
                        value: Box::new(value),
                    }
                }
                TokenKind::Yield => {
                    state.next();

                    if state.current.kind == TokenKind::SemiColon {
                        Expression::Yield {
                            key: None,
                            value: None,
                        }
                    } else {
                        let mut from = false;

                        if state.current.kind == TokenKind::From {
                            state.next();
                            from = true;
                        }

                        let mut key = None;
                        let mut value = Box::new(self.expression(
                            state,
                            if from {
                                Precedence::YieldFrom
                            } else {
                                Precedence::Yield
                            },
                        )?);

                        if state.current.kind == TokenKind::DoubleArrow && !from {
                            state.next();
                            key = Some(value.clone());
                            value = Box::new(self.expression(state, Precedence::Yield)?);
                        }

                        if from {
                            Expression::YieldFrom { value }
                        } else {
                            Expression::Yield {
                                key,
                                value: Some(value),
                            }
                        }
                    }
                }
                TokenKind::Clone => {
                    state.next();

                    let target = self.expression(state, Precedence::CloneOrNew)?;

                    Expression::Clone {
                        target: Box::new(target),
                    }
                }
                TokenKind::Variable(_) => Expression::Variable(identifiers::var(state)?),
                TokenKind::LiteralInteger(i) => {
                    let e = Expression::LiteralInteger { i: i.clone() };
                    state.next();
                    e
                }
                TokenKind::LiteralFloat(f) => {
                    let f = Expression::LiteralFloat { f: f.clone() };
                    state.next();
                    f
                }
                TokenKind::Identifier(_)
                | TokenKind::QualifiedIdentifier(_)
                | TokenKind::FullyQualifiedIdentifier(_) => {
                    Expression::Identifier(identifiers::full_name(state)?)
                }
                TokenKind::Self_ => {
                    if !state.has_class_scope {
                        return Err(ParseError::CannotFindTypeInCurrentScope(
                            state.current.kind.to_string(),
                            state.current.span,
                        ));
                    }

                    state.next();

                    self.postfix(state, Expression::Self_, &TokenKind::DoubleColon)?
                }
                TokenKind::Static => {
                    if !state.has_class_scope {
                        return Err(ParseError::CannotFindTypeInCurrentScope(
                            state.current.kind.to_string(),
                            state.current.span,
                        ));
                    }

                    state.next();

                    self.postfix(state, Expression::Static, &TokenKind::DoubleColon)?
                }
                TokenKind::Parent => {
                    if !state.has_class_parent_scope {
                        return Err(ParseError::CannotFindTypeInCurrentScope(
                            state.current.kind.to_string(),
                            state.current.span,
                        ));
                    }

                    state.next();

                    self.postfix(state, Expression::Parent, &TokenKind::DoubleColon)?
                }
                TokenKind::LiteralString(s) => {
                    let e = Expression::LiteralString { value: s.clone() };
                    state.next();
                    e
                }
                TokenKind::StringPart(_) => self.interpolated_string(state)?,
                TokenKind::StartDocString(_, kind) => {
                    let kind = *kind;

                    self.doc_string(state, kind)?
                }
                TokenKind::Backtick => self.shell_exec(state)?,
                TokenKind::True => {
                    let e = Expression::Bool { value: true };
                    state.next();
                    e
                }
                TokenKind::False => {
                    let e = Expression::Bool { value: false };
                    state.next();
                    e
                }
                TokenKind::Null => {
                    state.next();
                    Expression::Null
                }
                TokenKind::LeftParen => {
                    state.next();

                    let e = self.expression(state, Precedence::Lowest)?;

                    utils::skip_right_parenthesis(state)?;

                    e
                }
                TokenKind::Match => {
                    state.next();
                    utils::skip_left_parenthesis(state)?;

                    let condition = Box::new(self.expression(state, Precedence::Lowest)?);

                    utils::skip_right_parenthesis(state)?;
                    utils::skip_left_brace(state)?;

                    let mut default = None;
                    let mut arms = Vec::new();
                    while state.current.kind != TokenKind::RightBrace {
                        state.skip_comments();

                        if state.current.kind == TokenKind::Default {
                            if default.is_some() {
                                return Err(ParseError::MatchExpressionWithMultipleDefaultArms(
                                    state.current.span,
                                ));
                            }

                            state.next();

                            // match conditions can have an extra comma at the end, including `default`.
                            if state.current.kind == TokenKind::Comma {
                                state.next();
                            }

                            utils::skip_double_arrow(state)?;

                            let body = self.expression(state, Precedence::Lowest)?;

                            default = Some(Box::new(DefaultMatchArm { body }));
                        } else {
                            let mut conditions = Vec::new();
                            while state.current.kind != TokenKind::DoubleArrow {
                                conditions.push(self.expression(state, Precedence::Lowest)?);

                                if state.current.kind == TokenKind::Comma {
                                    state.next();
                                } else {
                                    break;
                                }
                            }

                            if !conditions.is_empty() {
                                utils::skip_double_arrow(state)?;
                            } else {
                                break;
                            }

                            let body = self.expression(state, Precedence::Lowest)?;

                            arms.push(MatchArm { conditions, body });
                        }

                        if state.current.kind == TokenKind::Comma {
                            state.next();
                        } else {
                            break;
                        }
                    }

                    utils::skip_right_brace(state)?;

                    Expression::Match {
                        condition,
                        default,
                        arms,
                    }
                }
                TokenKind::Array => {
                    let mut items = vec![];

                    state.next();

                    utils::skip_left_parenthesis(state)?;

                    while state.current.kind != TokenKind::RightParen {
                        let mut key = None;
                        let unpack = if state.current.kind == TokenKind::Ellipsis {
                            state.next();
                            true
                        } else {
                            false
                        };

                        let (mut by_ref, amper_span) = if state.current.kind == TokenKind::Ampersand
                        {
                            let span = state.current.span;
                            state.next();
                            (true, span)
                        } else {
                            (false, (0, 0))
                        };

                        let mut value = self.expression(state, Precedence::Lowest)?;

                        // TODO: return error for `[...$a => $b]`.
                        if state.current.kind == TokenKind::DoubleArrow {
                            state.next();

                            if by_ref {
                                return Err(ParseError::UnexpectedToken(
                                    TokenKind::Ampersand.to_string(),
                                    amper_span,
                                ));
                            }

                            key = Some(value);

                            by_ref = if state.current.kind == TokenKind::Ampersand {
                                state.next();
                                true
                            } else {
                                false
                            };

                            value = self.expression(state, Precedence::Lowest)?;
                        }

                        items.push(ArrayItem {
                            key,
                            value,
                            unpack,
                            by_ref,
                        });

                        if state.current.kind == TokenKind::Comma {
                            state.next();
                        } else {
                            break;
                        }

                        state.skip_comments();
                    }

                    utils::skip_right_parenthesis(state)?;

                    Expression::Array { items }
                }
                TokenKind::LeftBracket => self.array_expression(state)?,
                TokenKind::New => {
                    state.next();

                    let mut args = vec![];
                    let target = self.expression(state, Precedence::CloneOrNew)?;

                    if state.current.kind == TokenKind::LeftParen {
                        args = self.args_list(state)?;
                    }

                    Expression::New {
                        target: Box::new(target),
                        args,
                    }
                }
                TokenKind::DirConstant => {
                    state.next();
                    Expression::MagicConst {
                        constant: MagicConst::Dir,
                    }
                }
                TokenKind::Include
                | TokenKind::IncludeOnce
                | TokenKind::Require
                | TokenKind::RequireOnce => {
                    let kind: IncludeKind = (&state.current.kind).into();
                    state.next();

                    let path = self.expression(state, Precedence::Lowest)?;

                    Expression::Include {
                        kind,
                        path: Box::new(path),
                    }
                }
                _ if is_prefix(&state.current.kind) => {
                    let op = state.current.kind.clone();

                    state.next();

                    let rpred = Precedence::prefix(&op);
                    let rhs = self.expression(state, rpred)?;

                    prefix(&op, rhs)
                }
                TokenKind::Dollar => self.dynamic_variable(state)?,
                _ => {
                    return Err(ParseError::UnexpectedToken(
                        state.current.kind.to_string(),
                        state.current.span,
                    ))
                }
            }
        };

        if state.current.kind == TokenKind::SemiColon {
            return Ok(left);
        }

        state.skip_comments();

        loop {
            state.skip_comments();

            if matches!(state.current.kind, TokenKind::SemiColon | TokenKind::Eof) {
                break;
            }

            let span = state.current.span;
            let kind = state.current.kind.clone();

            if is_postfix(&kind) {
                let lpred = Precedence::postfix(&kind);

                if lpred < precedence {
                    break;
                }

                left = self.postfix(state, left, &kind)?;
                continue;
            }

            if is_infix(&kind) {
                let rpred = Precedence::infix(&kind);

                if rpred < precedence {
                    break;
                }

                if rpred == precedence && matches!(rpred.associativity(), Some(Associativity::Left))
                {
                    break;
                }

                if rpred == precedence && matches!(rpred.associativity(), Some(Associativity::Non))
                {
                    return Err(ParseError::UnexpectedToken(kind.to_string(), span));
                }

                state.next();

                match kind {
                    TokenKind::Question => {
                        let then = self.expression(state, Precedence::Lowest)?;
                        utils::colon(state)?;
                        let otherwise = self.expression(state, rpred)?;
                        left = Expression::Ternary {
                            condition: Box::new(left),
                            then: Some(Box::new(then)),
                            r#else: Box::new(otherwise),
                        }
                    }
                    TokenKind::QuestionColon => {
                        let r#else = self.expression(state, Precedence::Lowest)?;
                        left = Expression::Ternary {
                            condition: Box::new(left),
                            then: None,
                            r#else: Box::new(r#else),
                        }
                    }
                    _ => {
                        // FIXME: Hacky, should probably be refactored.
                        let by_ref =
                            kind == TokenKind::Equals && state.current.kind == TokenKind::Ampersand;
                        if by_ref {
                            state.next();
                        }

                        let rhs = self.expression(state, rpred)?;

                        left = infix(left, kind, rhs, by_ref);
                    }
                }

                continue;
            }

            break;
        }

        state.skip_comments();

        Ok(left)
    }

    fn postfix(
        &self,
        state: &mut State,
        lhs: Expression,
        op: &TokenKind,
    ) -> Result<Expression, ParseError> {
        Ok(match op {
            TokenKind::Coalesce => {
                state.next();

                let rhs = self.expression(state, Precedence::NullCoalesce)?;

                Expression::Coalesce {
                    lhs: Box::new(lhs),
                    rhs: Box::new(rhs),
                }
            }
            TokenKind::LeftParen => {
                let args = self.args_list(state)?;

                Expression::Call {
                    target: Box::new(lhs),
                    args,
                }
            }
            TokenKind::LeftBracket => {
                utils::skip_left_bracket(state)?;

                if state.current.kind == TokenKind::RightBracket {
                    state.next();

                    Expression::ArrayIndex {
                        array: Box::new(lhs),
                        index: None,
                    }
                } else {
                    let index = self.expression(state, Precedence::Lowest)?;

                    utils::skip_right_bracket(state)?;

                    Expression::ArrayIndex {
                        array: Box::new(lhs),
                        index: Some(Box::new(index)),
                    }
                }
            }
            TokenKind::DoubleColon => {
                utils::skip_double_colon(state)?;

                let mut must_be_method_call = false;

                let property = match state.current.kind.clone() {
                    TokenKind::Dollar => self.dynamic_variable(state)?,
                    TokenKind::Variable(_) => Expression::Variable(identifiers::var(state)?),
                    TokenKind::Identifier(_) => Expression::Identifier(identifiers::ident(state)?),
                    TokenKind::LeftBrace => {
                        must_be_method_call = true;
                        state.next();

                        let name = self.expression(state, Precedence::Lowest)?;

                        utils::skip_right_brace(state)?;

                        Expression::DynamicVariable {
                            name: Box::new(name),
                        }
                    }
                    TokenKind::Class => {
                        let start = state.current.span;
                        state.next();
                        let end = state.current.span;

                        Expression::Identifier(Identifier {
                            start,
                            name: "class".into(),
                            end,
                        })
                    }
                    _ if is_reserved_ident(&state.current.kind) => {
                        Expression::Identifier(identifiers::ident_maybe_reserved(state)?)
                    }
                    _ => {
                        return expected_token_err!(["`{`", "`$`", "an identifier"], state);
                    }
                };

                let lhs = Box::new(lhs);

                match property {
                    // 1. If we have an identifier and the current token is not a left paren,
                    //    the resulting expression must be a constant fetch.
                    Expression::Identifier(identifier)
                        if state.current.kind != TokenKind::LeftParen =>
                    {
                        Expression::ConstFetch {
                            target: lhs,
                            constant: identifier,
                        }
                    }
                    // 2. If the current token is a left paren, or if we know the property expression
                    //    is only valid a method call context, we can assume we're parsing a static
                    //    method call.
                    _ if state.current.kind == TokenKind::LeftParen || must_be_method_call => {
                        let args = self.args_list(state)?;

                        Expression::StaticMethodCall {
                            target: lhs,
                            method: Box::new(property),
                            args,
                        }
                    }
                    // 3. If we haven't met any of the previous conditions, we can assume
                    //    that we're parsing a static property fetch.
                    _ => Expression::StaticPropertyFetch {
                        target: lhs,
                        property: Box::new(property),
                    },
                }
            }
            TokenKind::Arrow | TokenKind::NullsafeArrow => {
                state.next();

                let property = match state.current.kind {
                    TokenKind::LeftBrace => {
                        utils::skip_left_brace(state)?;
                        let expr = self.expression(state, Precedence::Lowest)?;
                        utils::skip_right_brace(state)?;
                        expr
                    }
                    TokenKind::Variable(_) => Expression::Variable(identifiers::var(state)?),
                    TokenKind::Dollar => self.dynamic_variable(state)?,
                    _ => Expression::Identifier(identifiers::ident_maybe_reserved(state)?),
                };

                if state.current.kind == TokenKind::LeftParen {
                    let args = self.args_list(state)?;

                    if op == &TokenKind::NullsafeArrow {
                        Expression::NullsafeMethodCall {
                            target: Box::new(lhs),
                            method: Box::new(property),
                            args,
                        }
                    } else {
                        Expression::MethodCall {
                            target: Box::new(lhs),
                            method: Box::new(property),
                            args,
                        }
                    }
                } else if op == &TokenKind::NullsafeArrow {
                    Expression::NullsafePropertyFetch {
                        target: Box::new(lhs),
                        property: Box::new(property),
                    }
                } else {
                    Expression::PropertyFetch {
                        target: Box::new(lhs),
                        property: Box::new(property),
                    }
                }
            }
            TokenKind::Increment => {
                state.next();
                Expression::Increment {
                    value: Box::new(lhs),
                }
            }
            TokenKind::Decrement => {
                state.next();

                Expression::Decrement {
                    value: Box::new(lhs),
                }
            }
            _ => todo!("postfix: {:?}", op),
        })
    }

    #[inline(always)]
    fn interpolated_string(&self, state: &mut State) -> ParseResult<Expression> {
        let mut parts = Vec::new();

        while state.current.kind != TokenKind::DoubleQuote {
            if let Some(part) = self.interpolated_string_part(state)? {
                parts.push(part);
            }
        }

        state.next();

        Ok(Expression::InterpolatedString { parts })
    }

    #[inline(always)]
    fn shell_exec(&self, state: &mut State) -> ParseResult<Expression> {
        state.next();

        let mut parts = Vec::new();

        while state.current.kind != TokenKind::Backtick {
            if let Some(part) = self.interpolated_string_part(state)? {
                parts.push(part);
            }
        }

        state.next();

        Ok(Expression::ShellExec { parts })
    }

    #[inline(always)]
    fn doc_string(&self, state: &mut State, kind: DocStringKind) -> ParseResult<Expression> {
        state.next();

        Ok(match kind {
            DocStringKind::Heredoc => {
                let mut parts = Vec::new();

                while !matches!(state.current.kind, TokenKind::EndDocString(_, _, _)) {
                    if let Some(part) = self.interpolated_string_part(state)? {
                        parts.push(part);
                    }
                }

                let (indentation_type, indentation_amount) = match state.current.kind {
                    TokenKind::EndDocString(_, indentation_type, indentation_amount) => {
                        (indentation_type, indentation_amount)
                    }
                    _ => unreachable!(),
                };

                state.next();

                // FIXME: Can we move this logic above into the loop, by peeking ahead in
                //        the token stream for the EndHeredoc? Might be more performant.
                if let Some(indentation_type) = indentation_type {
                    let search_char: u8 = indentation_type.into();

                    for part in parts.iter_mut() {
                        match part {
                            StringPart::Const(bytes) => {
                                for _ in 0..indentation_amount {
                                    if bytes.starts_with(&[search_char]) {
                                        bytes.remove(0);
                                    }
                                }
                            }
                            _ => continue,
                        }
                    }
                }

                Expression::Heredoc { parts }
            }
            DocStringKind::Nowdoc => {
                // FIXME: This feels hacky. We should probably produce different tokens from the lexer
                //        but since I already had the logic in place for parsing heredocs, this was
                //        the fastest way to get nowdocs working too.
                let mut s = expect_token!([
                    TokenKind::StringPart(s) => s
                ], state, "constant string");

                let (indentation_type, indentation_amount) = expect_token!([
                    TokenKind::EndDocString(_, indentation_type, indentation_amount) => (indentation_type, indentation_amount)
                ], state, "label");

                // FIXME: Hacky code, but it's late and I want to get this done.
                if let Some(indentation_type) = indentation_type {
                    let search_char: u8 = indentation_type.into();
                    let mut lines = s
                        .split(|b| *b == b'\n')
                        .map(|s| s.to_vec())
                        .collect::<Vec<Vec<u8>>>();
                    for line in lines.iter_mut() {
                        for _ in 0..indentation_amount {
                            if line.starts_with(&[search_char]) {
                                line.remove(0);
                            }
                        }
                    }
                    let mut bytes = Vec::new();
                    for (i, line) in lines.iter().enumerate() {
                        bytes.extend(line);
                        if i < lines.len() - 1 {
                            bytes.push(b'\n');
                        }
                    }
                    s = bytes.into();
                }

                Expression::Nowdoc { value: s }
            }
        })
    }

    fn interpolated_string_part(&self, state: &mut State) -> ParseResult<Option<StringPart>> {
        Ok(match &state.current.kind {
            TokenKind::StringPart(s) => {
                let part = if s.len() > 0 {
                    Some(StringPart::Const(s.clone()))
                } else {
                    None
                };

                state.next();
                part
            }
            TokenKind::DollarLeftBrace => {
                state.next();
                let e = match (state.current.kind.clone(), state.peek.kind.clone()) {
                    (TokenKind::Identifier(name), TokenKind::RightBrace) => {
                        let start = state.current.span;
                        let end = state.peek.span;

                        state.next();
                        state.next();
                        // "${var}"
                        // TODO: we should use a different node for this.
                        Expression::Variable(Variable { start, name, end })
                    }
                    (TokenKind::Identifier(name), TokenKind::LeftBracket) => {
                        let start = state.current.span;
                        let end = state.peek.span;
                        state.next();
                        state.next();
                        let var = Expression::Variable(Variable { start, name, end });

                        let e = self.expression(state, Precedence::Lowest)?;
                        utils::skip_right_bracket(state)?;
                        utils::skip_right_brace(state)?;

                        // TODO: we should use a different node for this.
                        Expression::ArrayIndex {
                            array: Box::new(var),
                            index: Some(Box::new(e)),
                        }
                    }
                    _ => {
                        // Arbitrary expressions are allowed, but are treated as variable variables.
                        let e = self.expression(state, Precedence::Lowest)?;
                        utils::skip_right_brace(state)?;

                        Expression::DynamicVariable { name: Box::new(e) }
                    }
                };
                Some(StringPart::Expr(Box::new(e)))
            }
            TokenKind::LeftBrace => {
                // "{$expr}"
                state.next();
                let e = self.expression(state, Precedence::Lowest)?;
                utils::skip_right_brace(state)?;
                Some(StringPart::Expr(Box::new(e)))
            }
            TokenKind::Variable(_) => {
                // "$expr", "$expr[0]", "$expr[name]", "$expr->a"
                let var = Expression::Variable(identifiers::var(state)?);
                let e = match state.current.kind {
                    TokenKind::LeftBracket => {
                        state.next();
                        // Full expression syntax is not allowed here,
                        // so we can't call self.expression.
                        let index = match &state.current.kind {
                            TokenKind::LiteralInteger(i) => {
                                let e = Expression::LiteralInteger { i: i.clone() };
                                state.next();
                                e
                            }
                            TokenKind::Minus => {
                                state.next();
                                if let TokenKind::LiteralInteger(i) = &state.current.kind {
                                    let e = Expression::Negate {
                                        value: Box::new(Expression::LiteralInteger {
                                            i: i.clone(),
                                        }),
                                    };
                                    state.next();
                                    e
                                } else {
                                    return expected_token_err!("an integer", state);
                                }
                            }
                            TokenKind::Identifier(ident) => {
                                let e = Expression::LiteralString {
                                    value: ident.clone(),
                                };
                                state.next();
                                e
                            }
                            TokenKind::Variable(_) => {
                                let v = identifiers::var(state)?;
                                Expression::Variable(v)
                            }
                            _ => {
                                return expected_token_err!(
                                    ["`-`", "an integer", "an identifier", "a variable"],
                                    state
                                );
                            }
                        };

                        utils::skip_right_bracket(state)?;

                        Expression::ArrayIndex {
                            array: Box::new(var),
                            index: Some(Box::new(index)),
                        }
                    }
                    TokenKind::Arrow => {
                        state.next();
                        Expression::PropertyFetch {
                            target: Box::new(var),
                            property: Box::new(Expression::Identifier(
                                identifiers::ident_maybe_reserved(state)?,
                            )),
                        }
                    }
                    TokenKind::NullsafeArrow => {
                        state.next();
                        Expression::NullsafePropertyFetch {
                            target: Box::new(var),
                            property: Box::new(Expression::Identifier(
                                identifiers::ident_maybe_reserved(state)?,
                            )),
                        }
                    }
                    _ => var,
                };
                Some(StringPart::Expr(Box::new(e)))
            }
            _ => {
                return expected_token_err!(["`${`", "`{$", "`\"`", "a variable"], state);
            }
        })
    }
}

#[inline(always)]
fn is_prefix(op: &TokenKind) -> bool {
    matches!(
        op,
        TokenKind::Bang
            | TokenKind::Print
            | TokenKind::BitwiseNot
            | TokenKind::Decrement
            | TokenKind::Increment
            | TokenKind::Minus
            | TokenKind::Plus
            | TokenKind::StringCast
            | TokenKind::BinaryCast
            | TokenKind::ObjectCast
            | TokenKind::BoolCast
            | TokenKind::BooleanCast
            | TokenKind::IntCast
            | TokenKind::IntegerCast
            | TokenKind::FloatCast
            | TokenKind::DoubleCast
            | TokenKind::RealCast
            | TokenKind::UnsetCast
            | TokenKind::ArrayCast
            | TokenKind::At
    )
}

#[inline(always)]
fn prefix(op: &TokenKind, rhs: Expression) -> Expression {
    match op {
        TokenKind::Print => Expression::Print {
            value: Box::new(rhs),
        },
        TokenKind::Bang => Expression::BooleanNot {
            value: Box::new(rhs),
        },
        TokenKind::Minus => Expression::Negate {
            value: Box::new(rhs),
        },
        TokenKind::Plus => Expression::UnaryPlus {
            value: Box::new(rhs),
        },
        TokenKind::BitwiseNot => Expression::BitwiseNot {
            value: Box::new(rhs),
        },
        TokenKind::Decrement => Expression::PreDecrement {
            value: Box::new(rhs),
        },
        TokenKind::Increment => Expression::PreIncrement {
            value: Box::new(rhs),
        },
        TokenKind::StringCast
        | TokenKind::BinaryCast
        | TokenKind::ObjectCast
        | TokenKind::BoolCast
        | TokenKind::BooleanCast
        | TokenKind::IntCast
        | TokenKind::IntegerCast
        | TokenKind::FloatCast
        | TokenKind::DoubleCast
        | TokenKind::RealCast
        | TokenKind::UnsetCast
        | TokenKind::ArrayCast => Expression::Cast {
            kind: op.into(),
            value: Box::new(rhs),
        },
        TokenKind::At => Expression::ErrorSuppress {
            expr: Box::new(rhs),
        },
        _ => unreachable!(),
    }
}

#[inline(always)]
fn infix(lhs: Expression, op: TokenKind, rhs: Expression, by_ref: bool) -> Expression {
    Expression::Infix {
        lhs: Box::new(lhs),
        op: match (&op, by_ref) {
            (TokenKind::Equals, true) => ast::InfixOp::AssignRef,
            _ => op.into(),
        },
        rhs: Box::new(rhs),
    }
}

fn is_infix(t: &TokenKind) -> bool {
    matches!(
        t,
        TokenKind::Pow
            | TokenKind::RightShiftEquals
            | TokenKind::LeftShiftEquals
            | TokenKind::CaretEquals
            | TokenKind::AmpersandEquals
            | TokenKind::PipeEquals
            | TokenKind::PercentEquals
            | TokenKind::PowEquals
            | TokenKind::LogicalAnd
            | TokenKind::LogicalOr
            | TokenKind::LogicalXor
            | TokenKind::Spaceship
            | TokenKind::LeftShift
            | TokenKind::RightShift
            | TokenKind::Ampersand
            | TokenKind::Pipe
            | TokenKind::Caret
            | TokenKind::Percent
            | TokenKind::Instanceof
            | TokenKind::Asterisk
            | TokenKind::Slash
            | TokenKind::Plus
            | TokenKind::Minus
            | TokenKind::Dot
            | TokenKind::LessThan
            | TokenKind::GreaterThan
            | TokenKind::LessThanEquals
            | TokenKind::GreaterThanEquals
            | TokenKind::DoubleEquals
            | TokenKind::TripleEquals
            | TokenKind::BangEquals
            | TokenKind::BangDoubleEquals
            | TokenKind::AngledLeftRight
            | TokenKind::Question
            | TokenKind::QuestionColon
            | TokenKind::BooleanAnd
            | TokenKind::BooleanOr
            | TokenKind::Equals
            | TokenKind::PlusEquals
            | TokenKind::MinusEquals
            | TokenKind::DotEquals
            | TokenKind::CoalesceEqual
            | TokenKind::AsteriskEqual
            | TokenKind::SlashEquals
    )
}

#[inline(always)]
fn is_postfix(t: &TokenKind) -> bool {
    matches!(
        t,
        TokenKind::Increment
            | TokenKind::Decrement
            | TokenKind::LeftParen
            | TokenKind::LeftBracket
            | TokenKind::Arrow
            | TokenKind::NullsafeArrow
            | TokenKind::DoubleColon
            | TokenKind::Coalesce
    )
}
