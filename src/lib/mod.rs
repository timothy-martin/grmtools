// Copyright (c) 2017 King's College London
// created by the Software Development Team <http://soft-dev.org/>
//
// The Universal Permissive License (UPL), Version 1.0
//
// Subject to the condition set forth below, permission is hereby granted to any person obtaining a
// copy of this software, associated documentation and/or data (collectively the "Software"), free
// of charge and under any and all copyright rights in the Software, and any and all patent rights
// owned or freely licensable by each licensor hereunder covering either (i) the unmodified
// Software as contributed to or provided by such licensor, or (ii) the Larger Works (as defined
// below), to deal in both
//
// (a) the Software, and
// (b) any piece of software and/or hardware listed in the lrgrwrks.txt file
// if one is included with the Software (each a “Larger Work” to which the Software is contributed
// by such licensors),
//
// without restriction, including without limitation the rights to copy, create derivative works
// of, display, perform, and distribute the Software and make, use, sell, offer for sale, import,
// export, have made, and have sold the Software and the Larger Work(s), and to sublicense the
// foregoing rights on either these or other terms.
//
// This license is subject to the following condition: The above copyright notice and either this
// complete permission notice or at a minimum a reference to the UPL must be included in all copies
// or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR IMPLIED, INCLUDING
// BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE SOFTWARE.

use std::fmt;

#[macro_use] extern crate lazy_static;
#[macro_use] extern crate custom_derive;
#[macro_use] extern crate newtype_derive;


mod ast;
mod firsts;
pub mod grammar;
mod yacc_parser;
mod stategraph;
pub mod statetable;

pub use grammar::{Grammar, PIdx, NTIdx, Symbol, TIdx};
pub use ast::{GrammarAST, GrammarValidationError};
use stategraph::StateGraph;
pub use statetable::{Action, StateTable};
pub use yacc_parser::{YaccParserError, YaccParserErrorKind};
use yacc_parser::parse_yacc;

custom_derive! {
    /// A type specifically for state indexes.
    #[derive(Clone, Copy, Debug, Eq, Hash, NewtypeFrom, PartialEq)]
    pub struct StIdx(usize);
}

#[derive(Debug)]
pub enum YaccToStateTableError {
    YaccParserError(YaccParserError),
    GrammarValidationError(GrammarValidationError)
}

impl From<YaccParserError> for YaccToStateTableError {
    fn from(err: YaccParserError) -> YaccToStateTableError {
        YaccToStateTableError::YaccParserError(err)
    }
}

impl From<GrammarValidationError> for YaccToStateTableError {
    fn from(err: GrammarValidationError) -> YaccToStateTableError {
        YaccToStateTableError::GrammarValidationError(err)
    }
}

impl fmt::Display for YaccToStateTableError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            YaccToStateTableError::YaccParserError(ref e) => e.fmt(f),
            YaccToStateTableError::GrammarValidationError(ref e) => e.fmt(f),
        }
    }
}

pub enum Minimiser {
    Pager
}

pub fn yacc_to_statetable(s: &str, m: Minimiser) -> Result<(Grammar, StateTable), YaccToStateTableError> {
    let ast = try!(parse_yacc(s));
    try!(ast.validate());
    let grm = Grammar::new(&ast);
    let st = match m {
        Minimiser::Pager => {
            let sg = StateGraph::new(&grm);
            StateTable::new(&grm, &sg)
        }
    };
    Ok((grm, st))
}
