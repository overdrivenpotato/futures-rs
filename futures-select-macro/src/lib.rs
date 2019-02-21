//! The futures-rs `select! macro implementation.

#![recursion_limit="128"]
#![warn(rust_2018_idioms)]

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro_hack::proc_macro_hack;
use quote::quote;
use syn::{parenthesized, parse_quote, Expr, Ident, Pat, Token};
use syn::parse::{Parse, ParseStream};

mod kw {
    syn::custom_keyword!(complete);
    syn::custom_keyword!(futures_crate_path);
}

struct Select {
    futures_crate_path: Option<syn::Path>,
    // span of `complete`, then expression after `=> ...`
    complete: Option<Expr>,
    default: Option<Expr>,
    normal_fut_exprs: Vec<Expr>,
    normal_fut_handlers: Vec<(Pat, Expr)>,
}

#[allow(clippy::large_enum_variant)]
enum CaseKind {
    Complete,
    Default,
    Normal(Pat, Expr),
}

impl Parse for Select {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let mut select = Select {
            futures_crate_path: None,
            complete: None,
            default: None,
            normal_fut_exprs: vec![],
            normal_fut_handlers: vec![],
        };

        // When `futures_crate_path(::path::to::futures::lib)` is provided,
        // it sets the path through which futures library functions will be
        // accessed.
        if input.peek(kw::futures_crate_path) {
            input.parse::<kw::futures_crate_path>()?;
            let content;
            parenthesized!(content in input);
            select.futures_crate_path = Some(content.parse()?);
        }

        while !input.is_empty() {
            let case_kind = if input.peek(kw::complete) {
                // `complete`
                if select.complete.is_some() {
                    return Err(input.error("multiple `complete` cases found, only one allowed"));
                }
                input.parse::<kw::complete>()?;
                CaseKind::Complete
            } else if input.peek(Token![default]) {
                // `default`
                if select.default.is_some() {
                    return Err(input.error("multiple `default` cases found, only one allowed"));
                }
                input.parse::<Ident>()?;
                CaseKind::Default
            } else {
                // `<pat> = <expr>`
                let pat = input.parse()?;
                input.parse::<Token![=]>()?;
                let expr = input.parse()?;
                CaseKind::Normal(pat, expr)
            };

            // `=> <expr>`
            input.parse::<Token![=>]>()?;
            let expr = input.parse::<Expr>()?;

            // Commas after the expression are only optional if it's a `Block`
            // or it is the last branch in the `match`.
            let is_block = match expr { Expr::Block(_) => true, _ => false };
            if is_block || input.is_empty() {
                input.parse::<Option<Token![,]>>()?;
            } else {
                input.parse::<Token![,]>()?;
            }

            match case_kind {
                CaseKind::Complete => select.complete = Some(expr),
                CaseKind::Default => select.default = Some(expr),
                CaseKind::Normal(pat, fut_expr) => {
                    select.normal_fut_exprs.push(fut_expr);
                    select.normal_fut_handlers.push((pat, expr));
                },
            }
        }

        Ok(select)
    }
}

// Enum over all the cases in which the `select!` waiting has completed and the result
// can be processed.
//
// `enum __PrivResult<_1, _2, ...> { _1(_1), _2(_2), ..., Complete }`
fn declare_result_enum(
    result_ident: Ident,
    variants: usize,
    complete: bool,
    span: Span
) -> (Vec<Ident>, syn::ItemEnum) {
    // "_0", "_1", "_2"
    let variant_names: Vec<Ident> =
        (0..variants)
            .map(|num| Ident::new(&format!("_{}", num), span))
            .collect();

    let type_parameters = &variant_names;
    let variants = &variant_names;

    let complete_variant = if complete {
        Some(quote!(Complete))
    } else {
        None
    };

    let enum_item = parse_quote! {
        enum #result_ident<#(#type_parameters,)*> {
            #(
                #variants(#type_parameters),
            )*
            #complete_variant
        }
    };

    (variant_names, enum_item)
}

/// The `select!` macro.
#[proc_macro_hack]
pub fn select(input: TokenStream) -> TokenStream {
    let parsed = syn::parse_macro_input!(input as Select);

    let futures_crate: syn::Path = parsed.futures_crate_path.unwrap_or_else(|| parse_quote!(::futures_util));
    let rand_crate: syn::Path = parse_quote!(#futures_crate::rand_reexport);

    // should be def_site, but that's unstable
    let span = Span::call_site();

    let enum_ident = Ident::new("__PrivResult", span);

    let (variant_names, enum_item) = declare_result_enum(
        enum_ident.clone(),
        parsed.normal_fut_exprs.len(),
        parsed.complete.is_some(),
        span,
    );

    // bind non-`Ident` future exprs w/ `let`
    let mut future_let_bindings = Vec::with_capacity(parsed.normal_fut_exprs.len());
    let bound_future_names: Vec<_> = parsed.normal_fut_exprs.into_iter()
        .zip(variant_names.iter())
        .map(|(expr, variant_name)| {
            match expr {
                // Don't bind futures that are already a path.
                // This prevents creating redundant stack space
                // for them.
                syn::Expr::Path(path) => path,
                _ => {
                    future_let_bindings.push(quote! {
                        let mut #variant_name = #expr;
                    });
                    parse_quote! { #variant_name }
                }
            }
        })
        .collect();

    // For each future, make an `&mut dyn FnMut(&Waker) -> Option<Poll<__PrivResult<...>>`
    // to use for polling that individual future. These will then be put in an array.
    let poll_functions = bound_future_names.iter().zip(variant_names.iter())
        .map(|(bound_future_name, variant_name)| {
            quote! {
                let mut #variant_name = |__waker: &_| {
                    if #futures_crate::future::FusedFuture::is_terminated(&#bound_future_name) {
                        None
                    } else {
                        Some(#futures_crate::future::FutureExt::poll_unpin(
                            &mut #bound_future_name,
                            __waker,
                        ).map(#enum_ident::#variant_name))
                    }
                };
                let #variant_name: &mut dyn FnMut(
                    &#futures_crate::task::Waker
                ) -> Option<#futures_crate::task::Poll<_>> = &mut #variant_name;
            }
        });

    let none_polled = if parsed.complete.is_some() {
        quote! {
            #futures_crate::task::Poll::Ready(#enum_ident::Complete)
        }
    } else {
        quote! {
            panic!("all futures in select! were completed,\
                    but no `complete =>` handler was provided")
        }
    };

    let branches = parsed.normal_fut_handlers.into_iter()
        .zip(variant_names.iter())
        .map(|((pat, expr), variant_name)| {
            quote! {
                #enum_ident::#variant_name(#pat) => { #expr },
            }
        });
    let branches = quote! { #( #branches )* };

    let complete_branch = parsed.complete.map(|complete_expr| {
        quote! {
            #enum_ident::Complete => { #complete_expr },
        }
    });

    let branches = quote! {
        #branches
        #complete_branch
    };

    let await_and_select = if let Some(default_expr) = parsed.default {
        quote! {
            if let #futures_crate::task::Poll::Ready(x) =
                __poll_fn(#futures_crate::task::noop_waker_ref())
            {
                match x { #branches }
            } else {
                #default_expr
            };
        }
    } else {
        quote! {
            match r#await!(#futures_crate::future::poll_fn(__poll_fn)) {
                #branches
            }
        }
    };

    TokenStream::from(quote! { {
        #enum_item
        #( #future_let_bindings )*

        let mut __poll_fn = |__waker: &#futures_crate::task::Waker| {
            let mut __any_polled = false;

            #( #poll_functions )*

            let mut __select_arr = [#( #variant_names ),*];
            <[_] as #rand_crate::prelude::SliceRandom>::shuffle(
                &mut __select_arr,
                &mut #rand_crate::thread_rng(),
            );
            for poller in &mut __select_arr {
                let poller: &mut &mut dyn FnMut(
                    &#futures_crate::task::Waker
                ) -> Option<#futures_crate::task::Poll<_>> = poller;
                match poller(__waker) {
                    Some(x @ #futures_crate::task::Poll::Ready(_)) =>
                        return x,
                    Some(#futures_crate::task::Poll::Pending) => {
                        __any_polled = true;
                    }
                    None => {}
                }
            }

            if !__any_polled {
                #none_polled
            } else {
                #futures_crate::task::Poll::Pending
            }
        };

        #await_and_select
    } })
}
