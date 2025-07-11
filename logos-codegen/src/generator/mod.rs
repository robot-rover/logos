use std::fmt::Write;

use fnv::{FnvHashMap as Map, FnvHashSet as Set};
use proc_macro2::TokenStream;
use quote::{quote, ToTokens, TokenStreamExt};
use regex_automata::dfa::dense::DFA;
use regex_automata::util::primitives::StateID;
use syn::Ident;

use crate::graph::{Graph, State};
use crate::util::{MaybeVoid, ToIdent};

// mod context;
// mod fork;
mod leaf;
// mod rope;
// mod tables;

// use self::context::Context;
// use self::tables::TableStack;

pub struct Generator<'a> {
    /// Name of the type we are implementing the `Logos` trait for
    name: &'a Ident,
    /// Name of the type with any generics it might need
    this: &'a TokenStream,
    /// Reference to the graph with all of the nodes
    graph: &'a Graph<'a>,
    /// Function name identifiers
    idents: Map<State, Ident>,
}

impl<'a> Generator<'a> {
    pub fn new(
        name: &'a Ident,
        this: &'a TokenStream,
        graph: &'a Graph<'a>,
    ) -> Self {
        let mut idents = Map::default();

        for state in graph.get_states() {
            let mut name = format!("State{}", state.id.as_usize());
            if let Some(accept) = state.context {
                write!(name, "Ctx{}", accept.0).expect("Failed to write to string");
            }
            idents.insert(state, name.to_ident());
        }


        Generator {
            name,
            this,
            graph,
            idents,
        }
    }

    pub fn generate(mut self) -> TokenStream {
        // let root = self.goto(self.root, Context::default()).clone();
        // let tables = &self.tables;
        //
        let match_cases = self.graph.get_states().map(|state| self.generate_match_case(state)).collect::<Vec<_>>();

        for state in self.graph.get_states() {
            self.generate_match_case(state);
        }

        let init_state = &self.idents[&self.graph.root()];
        let all_idents = self.idents.values().collect::<Vec<_>>();

        quote! {
            #[derive(Clone, Copy)]
            enum LogosState {
                #(#all_idents),*
            }
            const START: LogosState = LogosState::#init_state;
            let mut state = START;
            let mut offset = lex.offset();
            loop {
                match state {
                    #(#match_cases)*
                }
            }
        }
    }

    fn get_ident(&self, state: &State) -> &Ident {
        self.idents.get(state).expect("Unreachable state found")
    }

    fn generate_match_case(&self, state: State) -> TokenStream {
        let this_ident = self.get_ident(&state);
        let mut setup = TokenStream::new();
        if state.is_accept {
            setup.append_all(quote! {
                lex.end(offset - 1);
            })
        };

        let mut inner_cases = TokenStream::new();
        let transitions = self.graph.get_transitions(&state);
        for (byte_class, next_state) in &transitions.normal {
            let next_ident = self.get_ident(&next_state);
            let patterns = byte_class.ranges.iter().map(|range| {
                let start = byte_to_tokens(*range.start());
                let end = byte_to_tokens(*range.end());
                if range.len() == 1 {
                    quote! { Some(#start) }
                } else {
                    quote! { Some(#start ..= #end) }
                }
            });
            inner_cases.append_all(quote! {
                #(#patterns)|* => {
                    offset += 1;
                    state = LogosState::#next_ident;
                },
            });
        }

        if state == self.graph.root() {
            inner_cases.append_all(quote!{ None => return None, });
        } else if let Some(eoi) = &transitions.eoi {
            let eoi_ident = self.get_ident(eoi);
            inner_cases.append_all(quote!{
                None => {
                    offset += 1;
                    state = LogosState::#eoi_ident;
                }
            });
        }

        let otherwise = if let Some(leaf_id) = state.context {
            self.generate_leaf(&self.graph.leaves()[leaf_id])
        } else {
            quote!{
                lex.error(offset);
                return Some(Err(Self::Error::default()));
            }
        };

                // println!("In state {} (lex: {}-{})", stringify!(#this_ident), lex.token_start, lex.token_end);
                // println!("Reading {:?}@{}", lex.read::<u8>(offset), offset);
        quote! {
            LogosState::#this_ident => {
                #setup
                match lex.read::<u8>(offset) {
                    #inner_cases
                    _ => { #otherwise }
                }
            }
        }
    }

}

macro_rules! match_quote {
    ($source:expr; $($byte:tt,)* ) => {match $source {
        $( $byte => quote!($byte), )*
        byte => quote!(#byte),
    }}
}

fn byte_to_tokens(byte: u8) -> TokenStream {
    match_quote! {
        byte;
        b'0', b'1', b'2', b'3', b'4', b'5', b'6', b'7', b'8', b'9',
        b'a', b'b', b'c', b'd', b'e', b'f', b'g', b'h', b'i', b'j',
        b'k', b'l', b'm', b'n', b'o', b'p', b'q', b'r', b's', b't',
        b'u', b'v', b'w', b'x', b'y', b'z',
        b'A', b'B', b'C', b'D', b'E', b'F', b'G', b'H', b'I', b'J',
        b'K', b'L', b'M', b'N', b'O', b'P', b'Q', b'R', b'S', b'T',
        b'U', b'V', b'W', b'X', b'Y', b'Z',
        b'!', b'@', b'#', b'$', b'%', b'^', b'&', b'*', b'(', b')',
        b'{', b'}', b'[', b']', b'<', b'>', b'-', b'=', b'_', b'+',
        b':', b';', b',', b'.', b'/', b'?', b'|', b'"', b'\'', b'\\',
    }
}
