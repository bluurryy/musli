use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::spanned::Spanned;

use crate::expander::{expand_tag, field_int};
use crate::expander::{
    Data, EnumData, Expander, ExpanderWithMode, ExpansionMode, FieldData, Result, StructData,
    TagMethod, TagMethods,
};
use crate::internals::attr::{DefaultTag, Packing};
use crate::internals::symbol::*;

pub(crate) fn expand_decode_entry(
    e: &Expander,
    expansion: ExpansionMode<'_>,
) -> Result<TokenStream> {
    let e = ExpanderWithMode {
        input: e.input,
        cx: &e.cx,
        type_attr: &e.type_attr,
        type_name: &e.type_name,
        data: &e.data,
        tokens: &e.tokens,
        mode: expansion.as_mode(&e.tokens),
    };

    expand_decode_moded(e, expansion)
}

/// Handle expansion of the `Decode` trait.
fn expand_decode_moded(
    e: ExpanderWithMode<'_>,
    expansion: ExpansionMode<'_>,
) -> Result<TokenStream> {
    e.validate_attributes()?;

    let span = e.input.ident.span();

    let root_decoder_var = syn::Ident::new("root_decoder", Span::call_site());

    let body = match &e.data {
        Data::Struct(data) => decode_struct(e, &root_decoder_var, data)?,
        Data::Enum(data) => decode_enum(e, &root_decoder_var, data)?,
        Data::Union => {
            e.cx.error_span(span, "musli: unions are not supported");
            return Err(());
        }
    };

    if e.cx.has_errors() {
        return Err(());
    }

    let mut impl_generics = e.input.generics.clone();
    let type_ident = &e.input.ident;

    let (lt, exists) = if let Some(existing) = impl_generics.lifetimes().next() {
        (existing.clone(), true)
    } else {
        let lt = syn::LifetimeDef::new(syn::Lifetime::new("'de", e.input.span()));
        (lt, false)
    };

    if !exists {
        impl_generics.params.push(lt.clone().into());
    }

    let decode_t = &e.tokens.decode_t;
    let decoder_t = &e.tokens.decoder_t;
    let original_generics = &e.input.generics;

    let (impl_generics, mode_ident, where_clause) =
        expansion.as_impl_generics(impl_generics, &e.tokens);

    Ok(quote_spanned! {
        span =>
        #[automatically_derived]
        impl #impl_generics #decode_t<#lt, #mode_ident> for #type_ident #original_generics #where_clause {
            #[inline]
            fn decode<D>(#root_decoder_var: D) -> Result<Self, D::Error>
            where
                D: #decoder_t<#lt>
            {
                #body
            }
        }
    })
}

fn decode_struct(
    e: ExpanderWithMode<'_>,
    decoder_var: &syn::Ident,
    data: &StructData,
) -> Result<TokenStream> {
    let path = syn::Path::from(syn::Ident::new("Self", e.input.ident.span()));
    let tag_type = e.type_attr.tag_type(e.mode);

    let body = match e.type_attr.packing(e.mode).cloned().unwrap_or_default() {
        Packing::Tagged => decode_tagged(
            e,
            data.span,
            decoder_var,
            &e.type_name,
            tag_type,
            path,
            &data.fields,
            None,
            e.type_attr.default_field_tag(e.mode),
        )?,
        Packing::Packed => decode_untagged(e, data.span, decoder_var, path, &data.fields)?,
        Packing::Transparent => decode_transparent(e, data.span, decoder_var, path, &data.fields)?,
    };

    Ok(quote! {
        Ok({ #body })
    })
}

fn decode_enum(
    e: ExpanderWithMode<'_>,
    root_decoder_var: &syn::Ident,
    data: &EnumData,
) -> Result<TokenStream> {
    let decoder_t = &e.tokens.decoder_t;
    let error_t = &e.tokens.error_t;
    let type_ident = &e.input.ident;
    let type_name = &e.type_name;
    let variant_tag = syn::Ident::new("variant_tag", data.span);
    let variant_decoder_var = syn::Ident::new("variant_decoder", data.span);
    let body_decoder_var = syn::Ident::new("body_decoder", data.span);

    if let Some(&(span, Packing::Packed)) = e.type_attr.packing_span(e.mode) {
        e.cx.error_span(
            span,
            format!(
                "`Decode` cannot be implemented on enums which are #[{}({})]",
                ATTR, PACKED
            ),
        );
        return Err(());
    }

    if data.variants.is_empty() {
        // Special case: Uninhabitable type. Since this cannot be reached, generate the never type.
        return Ok(quote! {
            Err(<D::Error as #error_t>::uninhabitable(#type_name))
        });
    }

    let type_packing = e.type_attr.packing(e.mode).cloned().unwrap_or_default();

    let tag_visitor_output = syn::Ident::new("VariantTagVisitorOutput", data.span);
    let mut string_patterns = Vec::with_capacity(data.variants.len());
    let mut output_variants = Vec::with_capacity(data.variants.len());
    // Collect variant names so that we can generate a debug implementation.
    let mut output_names = Vec::with_capacity(data.variants.len());

    let mut patterns = Vec::with_capacity(data.variants.len());
    let mut fallback = None;
    // Keep track of variant index manually since fallback variants do not
    // count.
    let mut variant_index = 0;
    let mut tag_methods = TagMethods::new(&e.cx);

    for v in data.variants.iter() {
        if v.attr.default_attr(e.mode).is_some() {
            if !v.fields.is_empty() {
                e.cx.error_span(
                    v.span,
                    format!("#[{}({})] variant must be empty", ATTR, DEFAULT),
                );
                continue;
            }

            if fallback.is_some() {
                e.cx.error_span(
                    v.span,
                    format!(
                        "#[{}({})] only one fallback variant is supported",
                        ATTR, DEFAULT
                    ),
                );
                continue;
            }

            fallback = Some(&v.ident);
            continue;
        }

        let mut path = syn::Path::from(syn::Ident::new("Self", type_ident.span()));
        path.segments.push(v.ident.clone().into());

        let tag_type = v.attr.tag_type(e.mode);

        let default_field_tag = v
            .attr
            .default_field_tag(e.mode)
            .unwrap_or_else(|| e.type_attr.default_field_tag(e.mode));

        let decode = match v.attr.packing(e.mode).unwrap_or(&type_packing) {
            Packing::Tagged => decode_tagged(
                e,
                v.span,
                &body_decoder_var,
                &v.name,
                tag_type,
                path,
                &v.fields,
                Some(&variant_tag),
                default_field_tag,
            )?,
            Packing::Packed => decode_untagged(e, v.span, &body_decoder_var, path, &v.fields)?,
            Packing::Transparent => {
                decode_transparent(e, v.span, &body_decoder_var, path, &v.fields)?
            }
        };

        let (tag, tag_method) = expand_tag(
            &e.cx,
            v.span,
            v.attr.rename(e.mode),
            e.type_attr.default_variant_tag(e.mode),
            variant_index,
            Some(&v.name),
        )?;

        tag_methods.insert(v.span, tag_method);

        let output_tag = handle_output_tag(
            v.span,
            variant_index,
            tag_method,
            &tag,
            &tag_visitor_output,
            &mut string_patterns,
            &mut output_variants,
        );

        output_names.push(&v.name);
        patterns.push((output_tag, decode));
        variant_index += 1;
    }

    let tag_type = e
        .type_attr
        .tag_type(e.mode)
        .as_ref()
        .map(|(_, ty)| quote!(: #ty));

    let fallback = match fallback {
        Some(ident) => {
            let variant_decoder_t = &e.tokens.variant_decoder_t;

            quote! {
                if !#variant_decoder_t::skip_variant(&mut #variant_decoder_var)? {
                    return Err(<D::Error as #error_t>::invalid_variant_tag(#type_name, tag));
                }

                #variant_decoder_t::end(#variant_decoder_var)?;
                Self::#ident {}
            }
        }
        None => quote! {
            return Err(<D::Error as #error_t>::invalid_variant_tag(#type_name, tag));
        },
    };

    let (decode_tag, unsupported_pattern, patterns, output_enum) = handle_tag_decode(
        e,
        &variant_decoder_var,
        tag_methods.pick(),
        &tag_visitor_output,
        &output_variants,
        &string_patterns,
        &patterns,
        &e.tokens.variant_decoder_t_tag,
    )?;

    // A `std::fmt::Debug` implementation is necessary for the output enum
    // since it is used to produce diagnostics.
    let output_enum_debug_impl = output_enum.is_some().then(|| {
        let fmt = &e.tokens.fmt;

        let mut patterns = Vec::new();

        for (name, variant) in output_names.iter().zip(output_variants.iter()) {
            patterns.push(quote!(#tag_visitor_output::#variant => #name.fmt(f)));
        }

        quote! {
            impl #fmt::Debug for #tag_visitor_output {
                #[inline]
                fn fmt(&self, f: &mut #fmt::Formatter<'_>) -> #fmt::Result {
                    match self { #(#patterns,)* #tag_visitor_output::Err(field) => field.fmt(f) }
                }
            }
        }
    });

    let patterns = patterns
        .into_iter()
        .map(|(tag, output)| {
            let variant_decoder_t = &e.tokens.variant_decoder_t;

            quote! {
                #tag => {
                    let output = {
                        let #body_decoder_var = #variant_decoder_t::variant(&mut #variant_decoder_var)?;
                        #output
                    };

                    #variant_decoder_t::end(#variant_decoder_var)?;
                    output
                }
            }
        })
        .collect::<Vec<_>>();

    Ok(quote! {
        let mut #variant_decoder_var = #decoder_t::decode_variant(#root_decoder_var)?;
        #output_enum
        #output_enum_debug_impl

        let #variant_tag #tag_type = #decode_tag;

        Ok(match #variant_tag {
            #(#patterns,)*
            #unsupported_pattern => {
                #fallback
            }
        })
    })
}

/// Decode something tagged.
///
/// If `variant_name` is specified it implies that a tagged enum is being
/// decoded.
fn decode_tagged(
    e: ExpanderWithMode<'_>,
    span: Span,
    parent_decoder_var: &syn::Ident,
    type_name: &syn::LitStr,
    tag_type: Option<&(Span, syn::Type)>,
    path: syn::Path,
    fields: &[FieldData],
    variant_tag: Option<&syn::Ident>,
    default_field_tag: DefaultTag,
) -> Result<TokenStream> {
    let struct_decoder_var = syn::Ident::new("struct_decoder", span);

    let decoder_t = &e.tokens.decoder_t;
    let error_t = &e.tokens.error_t;
    let default_t = &e.tokens.default_t;
    let pairs_decoder_t = &e.tokens.pairs_decoder_t;

    let fields_len = fields.len();
    let mut decls = Vec::with_capacity(fields_len);
    let mut patterns = Vec::with_capacity(fields_len);
    let mut assigns = Vec::with_capacity(fields_len);

    let tag_visitor_output = syn::Ident::new("TagVisitorOutput", e.input.ident.span());
    let mut string_patterns = Vec::with_capacity(fields_len);
    let mut output_variants = Vec::with_capacity(fields_len);

    let mut tag_methods = TagMethods::new(&e.cx);

    for field in fields {
        let (tag, tag_method) = expand_tag(
            &e.cx,
            field.span,
            field.attr.rename(e.mode),
            default_field_tag,
            field.index,
            field.name.as_ref(),
        )?;

        tag_methods.insert(field.span, tag_method);

        let (span, decode_path) = field.attr.decode_path(e.mode, field.span);
        let var = syn::Ident::new(&format!("v{}", field.index), span);
        decls.push(quote_spanned!(span => let mut #var = None;));
        let decode = quote_spanned!(span => #var = Some(#decode_path(#struct_decoder_var)?));

        let output_tag = handle_output_tag(
            span,
            field.index,
            tag_method,
            &tag,
            &tag_visitor_output,
            &mut string_patterns,
            &mut output_variants,
        );

        patterns.push((output_tag, decode));

        let field_ident = match &field.ident {
            Some(ident) => quote!(#ident),
            None => {
                let field_index = field_int(field.index, field.span);
                quote!(#field_index)
            }
        };

        let fallback = if let Some(span) = field.attr.default_attr(e.mode) {
            quote_spanned!(span => #default_t)
        } else {
            quote!(return Err(<D::Error as #error_t>::expected_tag(#type_name, #tag)))
        };

        assigns.push(quote!(#field_ident: match #var {
            Some(#var) => #var,
            None => #fallback,
        }));
    }

    let (decode_tag, unsupported_pattern, patterns, output_enum) = handle_tag_decode(
        e,
        &struct_decoder_var,
        tag_methods.pick(),
        &tag_visitor_output,
        &output_variants,
        &string_patterns,
        &patterns,
        &e.tokens.pair_decoder_t_first,
    )?;

    let pair_decoder_t = &e.tokens.pair_decoder_t;

    let patterns = patterns
        .into_iter()
        .map(|(tag, decode)| {
            quote! {
                #tag => {
                    let #struct_decoder_var = #pair_decoder_t::second(#struct_decoder_var)?;
                    #decode;
                }
            }
        })
        .collect::<Vec<_>>();

    let skip_field = quote! {
        #pair_decoder_t::skip_second(#struct_decoder_var)?
    };

    let unsupported = match variant_tag {
        Some(variant_tag) => quote! {
            <D::Error as #error_t>::invalid_variant_field_tag(#type_name, #variant_tag, tag)
        },
        None => quote! {
            <D::Error as #error_t>::invalid_field_tag(#type_name, tag)
        },
    };

    let tag_type = tag_type.as_ref().map(|(_, ty)| quote!(: #ty));

    let body = if patterns.is_empty() {
        quote! {
            if !#skip_field {
                return Err(#unsupported);
            }
        }
    } else {
        quote! {
            match tag {
                #(#patterns,)*
                #unsupported_pattern => {
                    if !#skip_field {
                        return Err(#unsupported);
                    }
                },
            }
        }
    };

    Ok(quote! {
        #(#decls;)*
        #output_enum
        let mut type_decoder = #decoder_t::decode_struct(#parent_decoder_var, #fields_len)?;

        while let Some(mut #struct_decoder_var) = #pairs_decoder_t::next(&mut type_decoder)? {
            let tag #tag_type = #decode_tag;
            #body
        }

        #pairs_decoder_t::end(type_decoder)?;
        #path { #(#assigns),* }
    })
}

/// Decode a transparent value.
fn decode_transparent(
    e: ExpanderWithMode<'_>,
    span: Span,
    decoder_var: &syn::Ident,
    path: syn::Path,
    fields: &[FieldData],
) -> Result<TokenStream> {
    let ident = fields.iter().next();

    let (accessor, field) = match ident {
        Some(field) if fields.len() == 1 => {
            let accessor = match &field.ident {
                Some(ident) => quote!(#ident),
                None => quote!(0),
            };

            (accessor, field)
        }
        _ => {
            e.transparent_diagnostics(span, fields);
            return Err(());
        }
    };

    let (span, decode_path) = field.attr.decode_path(e.mode, span);

    Ok(quote_spanned! {
        span =>
        #path {
            #accessor: #decode_path(#decoder_var)?
        }
    })
}

/// Decode something packed.
fn decode_untagged(
    e: ExpanderWithMode<'_>,
    span: Span,
    decoder_var: &syn::Ident,
    path: syn::Path,
    fields: &[FieldData],
) -> Result<TokenStream> {
    let decoder_t = &e.tokens.decoder_t;
    let pack_decoder_t = &e.tokens.pack_decoder_t;

    let mut assign = Vec::new();

    for field in fields {
        if let Some(span) = field.attr.default_attr(e.mode) {
            e.cx.error_span(
                span,
                format!(
                    "#[{}({})] fields cannot be used in an packed container",
                    ATTR, DEFAULT
                ),
            );
        }

        let (span, decode_path) = field.attr.decode_path(e.mode, field.span);

        let decode = quote! {{
            let field_decoder = #pack_decoder_t::next(&mut unpack)?;
            #decode_path(field_decoder)?
        }};

        match field.ident {
            Some(ident) => {
                let mut ident = ident.clone();
                ident.set_span(span);
                assign.push(quote_spanned!(field.span => #ident: #decode));
            }
            None => {
                let field_index = field_int(field.index, span);
                assign.push(quote_spanned!(field.span => #field_index: #decode));
            }
        }
    }

    if assign.is_empty() {
        Ok(quote_spanned!(span => #path {}))
    } else {
        Ok(quote_spanned! {
            span =>
            let mut unpack = #decoder_t::decode_pack(#decoder_var)?;
            let output = #path { #(#assign),* };
            #pack_decoder_t::end(unpack)?;
            output
        })
    }
}

/// Handle tag decoding.
fn handle_tag_decode(
    e: ExpanderWithMode<'_>,
    thing_decoder_var: &syn::Ident,
    tag_method: TagMethod,
    tag_visitor_output: &syn::Ident,
    output_variants: &[syn::Ident],
    string_patterns: &[TokenStream],
    patterns: &[(syn::Expr, TokenStream)],
    thing_decoder_t_decode: &syn::ExprPath,
) -> Result<(
    TokenStream,
    TokenStream,
    Vec<(TokenStream, TokenStream)>,
    Option<TokenStream>,
)> {
    match tag_method {
        TagMethod::String => {
            let (decode_tag, output_enum) = string_variant_tag_decode(
                e,
                thing_decoder_var,
                tag_visitor_output,
                output_variants,
                string_patterns,
                thing_decoder_t_decode,
            )?;

            let patterns = patterns
                .iter()
                .map(|(tag, decode)| (quote!(#tag), decode.clone()))
                .collect::<Vec<_>>();

            Ok((
                decode_tag,
                quote!(#tag_visitor_output::Err(tag)),
                patterns,
                Some(output_enum),
            ))
        }
        TagMethod::Default => {
            let decode_t_decode = e.mode.decode_t_decode();

            let decode_tag = quote! {{
                let index_decoder = #thing_decoder_t_decode(&mut #thing_decoder_var)?;
                #decode_t_decode(index_decoder)?
            }};

            let patterns = patterns
                .iter()
                .map(|(tag, decode)| (quote!(#tag), decode.clone()))
                .collect::<Vec<_>>();

            Ok((decode_tag, quote!(tag), patterns, None))
        }
    }
}

fn handle_output_tag(
    span: Span,
    index: usize,
    tag_method: TagMethod,
    tag: &syn::Expr,
    tag_visitor_output: &syn::Ident,
    string_patterns: &mut Vec<TokenStream>,
    output_variants: &mut Vec<syn::Ident>,
) -> syn::Expr {
    match tag_method {
        TagMethod::String => {
            let variant = syn::Ident::new(&format!("Variant{}", index), span);
            let mut path = syn::Path::from(tag_visitor_output.clone());
            path.segments.push(syn::PathSegment::from(variant.clone()));

            string_patterns.push(quote!(#tag => #path));
            output_variants.push(variant.clone());

            syn::Expr::Path(syn::ExprPath {
                attrs: Vec::new(),
                qself: None,
                path,
            })
        }
        TagMethod::Default => tag.clone(),
    }
}

fn string_variant_tag_decode(
    e: ExpanderWithMode<'_>,
    decoder_var: &syn::Ident,
    output: &syn::Ident,
    output_variants: &[syn::Ident],
    string_patterns: &[TokenStream],
    thing_decoder_t_decode: &syn::ExprPath,
) -> Result<(TokenStream, TokenStream)> {
    let decoder_t = &e.tokens.decoder_t;
    let visit_string_fn = &e.tokens.visit_string_fn;

    // Declare a tag visitor, allowing string tags to be decoded by
    // decoders that owns the string.
    let decode_tag = quote! {{
        let index_decoder = #thing_decoder_t_decode(&mut #decoder_var)?;

        #decoder_t::decode_string(index_decoder, #visit_string_fn(|string| {
            Ok::<#output, D::Error>(match string { #(#string_patterns,)* _ => #output::Err(string.into())})
        }))?
    }};

    let output_enum = quote! {
        enum #output {
            #(#output_variants,)*
            Err(String),
        }
    };

    Ok((decode_tag, output_enum))
}