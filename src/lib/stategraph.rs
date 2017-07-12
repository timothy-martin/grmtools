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
// if one is included with the Software (each a "Larger Work" to which the Software is contributed
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

use std::collections::hash_map::HashMap;

use StIdx;
use cfgrammar::Symbol;
use itemset::Itemset;

#[derive(Debug)]
pub struct StateGraph {
    /// A vector of states
    states: Vec<Itemset>,
    /// For each state in `states`, edges is a hashmap from symbols to state offsets.
    edges: Vec<HashMap<Symbol, StIdx>>
}

impl StateGraph {
    pub(crate) fn new(states: Vec<Itemset>, edges: Vec<HashMap<Symbol, StIdx>>) -> StateGraph {
        StateGraph{states, edges}
    }

    /// Return the itemset for state `st_idx` or `None` if it doesn't exist.
    pub fn state(&self, st_idx: StIdx) -> Option<&Itemset> {
        self.states.get(usize::from(st_idx))
    }

    /// Return an iterator over all states in this `StateGraph`.
    pub fn iter_states<'a>(&'a self) -> impl Iterator<Item=&Itemset> + 'a {
        self.states.iter()
    }

    /// How many items does this `StateGraph` contain?
    pub fn all_items_len(&self) -> usize {
        self.states.iter().fold(0, |a, x| a + x.items.len())
    }

    /// How many states does this `StateGraph` contain?
    pub fn all_states_len(&self) -> usize {
        self.states.len()
    }

    /// Return the state pointed to by `sym` from `st_idx` or `None` otherwise.
    pub fn edge(&self, st_idx: StIdx, sym: Symbol) -> Option<StIdx> {
        self.edges.get(usize::from(st_idx))
                  .and_then(|x| x.get(&sym))
                  .cloned()
    }

    /// Return the edges for state `st_idx` or `None` if it doesn't exist.
    pub fn edges(&self, st_idx: StIdx) -> Option<&HashMap<Symbol, StIdx>> {
        self.edges.get(usize::from(st_idx))
    }

    /// How many edges does this `StateGraph` contain?
    pub fn all_edges_len(&self) -> usize {
        self.edges.iter().fold(0, |a, x| a + x.len())
    }
}

#[cfg(test)]
use cfgrammar::{Grammar, TIdx};
#[cfg(test)]
use cfgrammar::yacc::YaccGrammar;

#[cfg(test)]
pub fn state_exists(grm: &YaccGrammar, is: &Itemset, nt: &str, prod_off: usize, dot: usize, la:
                    Vec<&str>) {

    let ab_prod_off = grm.nonterm_to_prods(grm.nonterm_idx(nt).unwrap()).unwrap()[prod_off];
    let ctx = &is.items[&(ab_prod_off, dot.into())];
    for i in 0..grm.terms_len() {
        let bit = ctx[i];
        let mut found = false;
        for t in la.iter() {
            let off = if t == &"$" {
                    TIdx::from(grm.eof_term_idx())
                } else {
                    grm.term_idx(t).unwrap()
                };
            if off == i.into() {
                if !bit {
                    panic!("bit for terminal {}, dot {} is not set in production {} of {} when it should be",
                           t, dot, prod_off, nt);
                }
                found = true;
                break;
            }
        }
        if !found && bit {
            panic!("bit for terminal {}, dot {} is set in production {} of {} when it shouldn't be",
                   grm.term_name(i.into()).unwrap(), dot, prod_off, nt);
        }
    }
}
