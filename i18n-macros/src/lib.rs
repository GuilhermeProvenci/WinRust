use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{format_ident, quote};
use syn::{parse::{Parse, ParseStream}, parse_macro_input, punctuated::Punctuated, Expr, LitStr, Token};

struct Input { template: LitStr, args: Punctuated<Expr, Token![,]> }
impl Parse for Input {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let template = input.parse()?;
        let args = if input.is_empty() { Punctuated::new() } else {
            input.parse::<Token![,]>()?;
            Punctuated::parse_terminated(input)?
        };
        Ok(Self { template, args })
    }
}

/// Formats a translated template using values formatted by Rust itself. If a
/// catalog is absent or has incompatible placeholders, uses standard format!.
#[proc_macro]
pub fn localized_format(input: TokenStream) -> TokenStream {
    expand_format(parse_macro_input!(input as Input), None)
}
struct ContextInput { ru: LitStr, inner: Input }
impl Parse for ContextInput { fn parse(input: ParseStream<'_>) -> syn::Result<Self> { let ru = input.parse()?; input.parse::<Token![,]>()?; Ok(Self { ru, inner: input.parse()? }) } }
#[proc_macro]
pub fn localized_format_context(input: TokenStream) -> TokenStream {
    let ContextInput { ru, inner } = parse_macro_input!(input as ContextInput);
    let hash = ru.value().bytes().fold(14695981039346656037u64, |h, b| (h ^ b as u64).wrapping_mul(1099511628211));
    expand_format(inner, Some(hash))
}
fn expand_format(input: Input, context: Option<u64>) -> TokenStream {
    let Input { template, args } = input;
    let source = template.value();
    let mut fields = Vec::new();
    let mut positional = std::collections::HashSet::new();
    let mut unsupported = args.iter().any(|arg| matches!(arg, Expr::Assign(_)));
    let mut chars = source.chars().peekable();
    let mut implicit = 0usize;
    while let Some(c) = chars.next() {
        if c != '{' { if c == '}' && chars.peek() == Some(&'}') { chars.next(); } continue; }
        if chars.peek() == Some(&'{') { chars.next(); continue; }
        let mut field = String::new();
        for ch in chars.by_ref() { if ch == '}' { break; } field.push(ch); }
        let (name, spec) = field.split_once(':').map_or((field.as_str(), ""), |(n, s)| (n, s));
        let index = if name.is_empty() { let n = implicit; implicit += 1; Some(n) } else { name.parse::<usize>().ok() };
        if spec.contains('$') { unsupported = true; }
        if let Some(i) = index { if !positional.insert(i) { unsupported = true; } }
        let (key_name, expression) = if let Some(i) = index {
            if i >= args.len() { return syn::Error::new(template.span(), "missing format argument").to_compile_error().into(); }
            let arg = &args[i];
            (i.to_string(), quote!(#arg))
        } else {
            match syn::parse_str::<syn::Ident>(name) {
                Ok(mut id) => { id.set_span(template.span()); (name.to_string(), quote!(#id)) },
                Err(_) => return syn::Error::new(template.span(), "unsupported named placeholder").to_compile_error().into(),
            }
        };
        let fmt = LitStr::new(&format!("{{:{spec}}}"), Span::call_site());
        let key = LitStr::new(&format!("{key_name}:{spec}"), Span::call_site());
        let ident = format_ident!("__winrust_value_{}", fields.len());
        fields.push((ident, key, fmt, expression));
    }
    let values = fields.iter().map(|(id, _, fmt, expression)| quote!(let #id = format!(#fmt, &(#expression));));
    let entries = fields.iter().map(|(id, key, _, _)| quote!((#key, #id)));
    let original_args: Vec<_> = args.iter().collect();
    if unsupported { return quote!(format!(#template #(, #original_args)*)).into(); }
    let lookup = if let Some(hash) = context { quote!(::winrust_common::locales::lookup_context(#template, #hash)) } else { quote!(::winrust_common::locales::lookup_current(#template)) };
    quote!({
        if let Some(__template) = #lookup
            .filter(|translated| ::winrust_common::locales::compatible(#template, translated)) {
            #(#values)*
            match ::winrust_common::locales::render(__template, &[#(#entries),*]) {
                Some(value) => value,
                None => format!(#template #(, #original_args)*),
            }
        } else { format!(#template #(, #original_args)*) }
    }).into()
}
