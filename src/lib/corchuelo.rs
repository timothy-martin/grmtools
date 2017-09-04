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

use std::convert::{TryFrom, TryInto};
use std::fmt::Debug;

use cactus::Cactus;
use cfgrammar::{NTIdx, Symbol, TIdx};
use lrlex::Lexeme;
use lrtable::{Action, StIdx};

use parser::{Node, Parser, ParseRepair};

const PARSE_AT_LEAST: usize = 3; // N in Corchuelo et al.
const PORTION_THRESHOLD: usize = 10; // N_t in Corchuelo et al.
const INSERT_THRESHOLD: usize = 4; // N_i in Corchuelo et al.
const DELETE_THRESHOLD: usize = 3; // N_d in Corchuelo et al.

pub(crate) fn recover<TokId: Clone + Copy + Debug + TryFrom<usize> + TryInto<usize> + PartialEq>
                     (parser: &Parser<TokId>, in_la_idx: usize, in_pstack: &mut Vec<StIdx>,
                      tstack: &mut Vec<Node<TokId>>)
                  -> (usize, Vec<Vec<ParseRepair>>)
{
    // This function implements the algorithm from "Repairing syntax errors in LR parsers" by
    // Rafael Corchuelo, Jose A. Perez, Antonio Ruiz, and Miguel Toro.
    //
    // Because we want to create a parse tree even when error recovery has happened, we can be a
    // bit clever. In our first stage, we try and find repair sequences using a cactus stack to
    // represent the parse stack, but we don't try and create/alter the parse tree. Once we've
    // found valid repairs, we select one arbitrarily (as do Corchuelo) and then replay it, this
    // time turning on parse tree creation/alteration. Thus we only pay the costs of creating the
    // parse tree for the one parse that we need it. This has a vaguely similar flavour to part of
    // the ALL(*) algorithm (where, when the LL parser gets to a point of ambiguity, it fires up
    // non-LL sub-parsers, which then tell the LL parser which path it should take).

    let mut cactus_pstack = Cactus::new();
    for st in in_pstack.drain(..) {
        cactus_pstack = cactus_pstack.child(st);
    }
    let start_cactus_pstack = cactus_pstack.clone();

    let mut todo = vec![(in_la_idx, cactus_pstack, Cactus::new(), 0)];
    let mut finished = vec![];
    let mut finished_score: Option<usize> = None;
    while todo.len() > 0 {
        let cur = todo.remove(0);
        let la_idx = cur.0;
        let pstack = cur.1;
        let repairs: Cactus<ParseRepair> = cur.2;

        // Insertion rule (ER1)
        match repairs.val() {
            Some(&ParseRepair::Delete) => {
                // In order to avoid adding both [Del, Ins x] and [Ins x, Del] (which are
                // equivalent), we follow Corcheulo et al.'s suggestion and never add an Ins after
                // a Del.
            },
            _ => {
                let num_inserts = repairs.vals()
                                         .filter(|r| if let ParseRepair::Insert{..} = **r {
                                                         true
                                                     } else {
                                                         false
                                                     })
                                         .count();
                if num_inserts <= INSERT_THRESHOLD {
                    for sym in parser.stable.state_actions(*pstack.val().unwrap()) {
                        if let Symbol::Term(term_idx) = sym {
                            if term_idx == parser.grm.eof_term_idx() {
                                continue;
                            }

                            // We make the artificially inserted lexeme appear to start at the same
                            // position as the real next lexeme, but have zero length (so that it's
                            // clear it's not really something the user created).
                            let (next_lexeme, _) = parser.next_lexeme(None, la_idx);
                            let new_lexeme = Lexeme::new(TokId::try_from(usize::from(term_idx))
                                                                        .ok()
                                                                        .unwrap(),
                                                         next_lexeme.start(), 0);
                            let (new_la_idx, n_pstack) =
                                lr_cactus(parser, Some(new_lexeme), la_idx, la_idx + 1,
                                          pstack.clone(), None);
                            if new_la_idx > la_idx {
                                debug_assert_eq!(new_la_idx, la_idx + 1);
                                let n_repairs = repairs.child(ParseRepair::Insert{term_idx});
                                let sc = score(&n_repairs);
                                if finished_score.is_none() || sc <= finished_score.unwrap() {
                                    todo.push((la_idx, n_pstack, n_repairs, sc));
                                }
                            }
                        }
                    }
                }
            }
        }

        // Delete rule (ER2)
        if la_idx < parser.lexemes.len() {
            let num_deletes = repairs.vals()
                                     .filter(|r| if let ParseRepair::Delete = **r {
                                                     true
                                                 } else {
                                                     false
                                                 })
                                     .count();
            if num_deletes <= DELETE_THRESHOLD {
                let n_repairs = repairs.child(ParseRepair::Delete);
                let sc = score(&n_repairs);
                if finished_score.is_none() || sc <= finished_score.unwrap() {
                    todo.push((la_idx + 1, pstack.clone(), n_repairs, sc));
                }
            }
        }

        // Forward move rule (ER3)
        //
        // Note the rule in Corchuelo et al. is confusing and, I think, wrong. It reads:
        //   (S, I) \rightarrow_{LR*} (S', I') \wedge (j = N \vee 0 < j < N
        //                                             \wedge f(q_r, t_{j + 1} \in {accept, error})
        // First I think the bracketing would be clearer if written as:
        //   j = N \vee (0 < j < N \wedge f(q_r, t_{j + 1} \in {accept, error})
        // And I think the condition should be:
        //   j = N \vee (0 <= j < N \wedge f(q_r, t_{j + 1} \in {accept, error})
        // because there's no reason that any symbols need to be shifted in order for an accept
        // (or, indeed an error) state to be reached.
        //
        // So the full rule should I think be:
        //   (S, I) \rightarrow_{LR*} (S', I')
        //   \wedge (j = N \vee (0 <= j < N \wedge f(q_r, t_{j + 1} \in {accept, error}))
        {
            let (new_la_idx, n_pstack)
                = lr_cactus(parser, None, la_idx, la_idx + PARSE_AT_LEAST, pstack.clone(), None);
            if new_la_idx < in_la_idx + PORTION_THRESHOLD {
                let mut n_repairs = repairs.clone();
                for _ in la_idx..new_la_idx {
                    n_repairs = n_repairs.child(ParseRepair::Shift);
                }

                // A repair is a "finisher" (i.e. can be considered complete and doesn't need to be
                // added to the todo list) if it's parsed at least N symbols or parsing ends in
                // an Accept action.
                let mut finisher = false;
                if new_la_idx == la_idx + PARSE_AT_LEAST {
                    finisher = true;
                } else {
                    debug_assert!(new_la_idx < la_idx + PARSE_AT_LEAST);
                    let st = *n_pstack.val().unwrap();
                    let (_, la_term) = parser.next_lexeme(None, new_la_idx);
                    match parser.stable.action(st, la_term) {
                        Some(Action::Accept) => finisher = true,
                        None => (),
                        _ => continue,
                    }
                }

                if finisher {
                    let sc = score(&n_repairs);
                    if finished_score.is_none() || sc < finished_score.unwrap() {
                        finished_score = Some(sc);
                        finished.clear();
                        todo.retain(|x| score(&x.2) <= sc);
                    }
                    finished.push(n_repairs);
                } else if new_la_idx > la_idx {
                    let sc = score(&n_repairs);
                    if finished_score.is_none() || sc <= finished_score.unwrap() {
                        todo.push((new_la_idx, n_pstack, n_repairs, sc));
                    }
                }
            }
        }
    }

    let repairs = finished.iter()
                          .map(|x| { let mut v = x.vals().cloned().collect::<Vec<ParseRepair>>();
                                     v.reverse();
                                     v
                           })
                          .collect::<Vec<Vec<ParseRepair>>>();

    // Arbitrarily select one of the repairs and replay it against the original starting pstack,
    // this time also creating a parse tree.

    let mut la_idx = in_la_idx;
    {
        let mut pstack = start_cactus_pstack;
        for r in repairs[0].iter() {
            match *r {
                ParseRepair::Insert{term_idx} => {
                    let (next_lexeme, _) = parser.next_lexeme(None, la_idx);
                    let new_lexeme = Lexeme::new(TokId::try_from(usize::from(term_idx))
                                                                .ok()
                                                                .unwrap(),
                                                 next_lexeme.start(), 0);
                    pstack = lr_cactus(parser, Some(new_lexeme), la_idx, la_idx + 1,
                                       pstack, Some(tstack)).1;
                },
                ParseRepair::Delete => {
                    la_idx += 1;
                }
                ParseRepair::Shift => {
                    let (new_la_idx, n_pstack)
                        = lr_cactus(parser, None, la_idx, la_idx + PARSE_AT_LEAST, pstack, Some(tstack));
                    la_idx = new_la_idx;
                    pstack = n_pstack;
                }
            }
        }

        in_pstack.clear();
        while !pstack.is_empty() {
            let p = pstack.parent().unwrap();
            in_pstack.push(pstack.take_or_clone_val().unwrap());
            pstack = p;
        }
        in_pstack.reverse();
    }

    (la_idx, repairs)
}

/// Start parsing text at `la_idx` (using the lexeme in `lexeme_prefix`, if it is not `None`,
/// as the first lexeme) up to (but excluding) `end_la_idx`. If an error is encountered, parsing
/// immediately terminates (without recovery).
///
/// Note that if `lexeme_prefix` is specified, `la_idx` will still be incremented, and thus
/// `end_la_idx` *must* be set to `la_idx + 1` in order that the parser doesn't skip the real
/// lexeme at position `la_idx`.
fn lr_cactus<TokId: Clone + Copy + Debug + TryFrom<usize> + TryInto<usize> + PartialEq>
    (parser: &Parser<TokId>, lexeme_prefix: Option<Lexeme<TokId>>, mut la_idx: usize, end_la_idx: usize,
     mut pstack: Cactus<StIdx>, mut tstack: Option<&mut Vec<Node<TokId>>>)
  -> (usize, Cactus<StIdx>)
{
    assert!(lexeme_prefix.is_none() || end_la_idx == la_idx + 1);
    while la_idx != end_la_idx {
        let st = *pstack.val().unwrap();
        let (la_lexeme, la_term) = parser.next_lexeme(lexeme_prefix, la_idx);

        match parser.stable.action(st, la_term) {
            Some(Action::Reduce(prod_id)) => {
                let nonterm_idx = parser.grm.prod_to_nonterm(prod_id);
                let pop_num = parser.grm.prod(prod_id).unwrap().len();
                if let Some(ref mut tstack_uw) = tstack {
                    let nodes = tstack_uw.drain(pstack.len() - pop_num - 1..)
                                         .collect::<Vec<Node<TokId>>>();
                    tstack_uw.push(Node::Nonterm{nonterm_idx: nonterm_idx, nodes: nodes});
                }

                for _ in 0..pop_num {
                    pstack = pstack.parent().unwrap();
                }
                let prior = *pstack.val().unwrap();
                pstack = pstack.child(parser.stable.goto(prior, NTIdx::from(nonterm_idx)).unwrap());
            },
            Some(Action::Shift(state_id)) => {
                if let Some(ref mut tstack_uw) = tstack {
                    tstack_uw.push(Node::Term{lexeme: la_lexeme});
                }
                pstack = pstack.child(state_id);
                la_idx += 1;
            },
            Some(Action::Accept) => {
                debug_assert_eq!(la_term, Symbol::Term(TIdx::from(parser.grm.eof_term_idx())));
                if let Some(ref mut tstack_uw) = tstack {
                    debug_assert_eq!(tstack_uw.len(), 1);
                }
                break;
            },
            None => {
                break;
            }
        }
    }
    (la_idx, pstack)
}

fn score(repairs: &Cactus<ParseRepair>) -> usize {
    repairs.vals()
           .filter(|x| match *x {
                           &ParseRepair::Insert{..} | &ParseRepair::Delete => true,
                           &ParseRepair::Shift => false
                       })
           .count()
}

#[cfg(test)]
mod test {
    use cfgrammar::yacc::YaccGrammar;
    use std::convert::TryFrom;
    use lrlex::Lexeme;
    use parser::ParseRepair;
    use parser::test::do_parse;

    fn pp_repairs(grm: &YaccGrammar, repairs: &Vec<ParseRepair>) -> String {
        let mut out = vec![];
        for &r in repairs {
            match r {
                ParseRepair::Insert{term_idx} =>
                    out.push(format!("Insert \"{}\"", grm.term_name(term_idx).unwrap())),
                ParseRepair::Delete =>
                    out.push(format!("Delete")),
                ParseRepair::Shift =>
                    out.push(format!("Shift"))
            }
        }
        out.join(", ")
    }

    fn check_repairs(grm: &YaccGrammar, repairs: &Vec<Vec<ParseRepair>>, expected: &[&str]) {
        for i in 0..repairs.len() {
            // First of all check that all the repairs are unique
            for j in i + 1..repairs.len() {
                assert_ne!(repairs[i], repairs[j]);
            }
            expected.iter().find(|x| **x == pp_repairs(&grm, &repairs[i])).unwrap();
        }
    }

    #[test]
    fn corchuelo_example() {
        // The example from the Curchuelo paper
        let lexs = "%%
[(] OPEN_BRACKET
[)] CLOSE_BRACKET
[+] PLUS
n N
";
        let grms = "%start E
%%
E : 'N'
  | E 'PLUS' 'N'
  | 'OPEN_BRACKET' E 'CLOSE_BRACKET'
  ;
";

        let (grm, pr) = do_parse(&lexs, &grms, "(nn");
        let errs = pr.unwrap_err();
        assert_eq!(errs.len(), 1);
        let err_tok_id = u16::try_from(usize::from(grm.term_idx("N").unwrap())).ok().unwrap();
        assert_eq!(errs[0].lexeme(), &Lexeme::new(err_tok_id, 2, 1));
        assert_eq!(errs[0].repairs().len(), 3);
        check_repairs(&grm,
                      errs[0].repairs(),
                      &vec!["Insert \"CLOSE_BRACKET\", Insert \"PLUS\", Shift",
                            "Insert \"CLOSE_BRACKET\", Delete",
                            "Insert \"PLUS\", Shift, Insert \"CLOSE_BRACKET\""]);

        let (grm, pr) = do_parse(&lexs, &grms, "n)+n+n+n)");
        let errs = pr.unwrap_err();
        assert_eq!(errs.len(), 2);
        check_repairs(&grm,
                      errs[0].repairs(),
                      &vec!["Delete, Shift, Shift, Shift"]);
        check_repairs(&grm,
                      errs[1].repairs(),
                      &vec!["Delete, Delete, Delete, Delete",
                            "Delete"]);

        let (grm, pr) = do_parse(&lexs, &grms, "(((+n)+n+n+n)");
        let errs = pr.unwrap_err();
        assert_eq!(errs.len(), 2);
        check_repairs(&grm,
                      errs[0].repairs(),
                      &vec!["Insert \"N\", Shift, Shift, Shift",
                            "Delete, Shift, Shift, Shift"]);
        check_repairs(&grm,
                      errs[1].repairs(),
                      &vec!["Insert \"CLOSE_BRACKET\""]);
    }
}
