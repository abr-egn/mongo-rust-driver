extern crate proc_macro;

mod action_impl;
mod option;
mod rustdoc;

use macro_magic::import_tokens_attr;
use syn::{bracketed, parse::ParseStream, punctuated::Punctuated, Error, Ident, Token};

/// Generates:
/// * an `IntoFuture` executing the given method body
/// * an opaque wrapper type for the future in case we want to do something more fancy than
///   BoxFuture.
/// * a `run` method for sync execution, optionally with a wrapper function
#[proc_macro_attribute]
pub fn action_impl(
    attrs: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    crate::action_impl::action_impl(attrs, input)
}

/// Enables rustdoc links to types that link individually to each type
/// component.
#[proc_macro_attribute]
pub fn deeplink(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    crate::rustdoc::deeplink(attr, item)
}

#[import_tokens_attr]
#[with_custom_parsing(crate::option::OptionSettersArgs)]
#[proc_macro_attribute]
pub fn option_setters(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    crate::option::option_setters(attr, item, __custom_tokens)
}

#[proc_macro_attribute]
pub fn export_doc(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    crate::rustdoc::export_doc(attr, item)
}

#[import_tokens_attr]
#[with_custom_parsing(crate::rustdoc::OptionsDocArgs)]
#[proc_macro_attribute]
pub fn options_doc(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    crate::rustdoc::options_doc(attr, item, __custom_tokens)
}

/// Parse an identifier with a specific expected value.
fn parse_name(input: ParseStream, name: &str) -> syn::Result<Ident> {
    let ident = input.parse::<Ident>()?;
    if ident.to_string() != name {
        return Err(Error::new(
            ident.span(),
            format!("expected '{}', got '{}'", name, ident),
        ));
    }
    Ok(ident)
}

macro_rules! macro_error {
    ($span:expr, $($message:tt)+) => {{
        return Error::new($span, format!($($message)+)).into_compile_error().into();
    }};
}
use macro_error;

fn parse_ident_list(input: ParseStream, name: &str) -> syn::Result<Vec<Ident>> {
    parse_name(input, name)?;
    input.parse::<Token![=]>()?;
    let content;
    bracketed!(content in input);
    let punc = Punctuated::<Ident, Token![,]>::parse_terminated(&content)?;
    Ok(punc.into_pairs().map(|p| p.into_value()).collect())
}
