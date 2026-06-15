use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    parse_macro_input, Fields, FieldsUnnamed, ItemEnum, Type,
    punctuated::Punctuated,
    token::Paren,
};

pub fn responsible_macro(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut enum_def = parse_macro_input!(item as ItemEnum);

    for variant in enum_def.variants.iter_mut() {
        let mut found_ty: Option<Type> = None;
        let mut found_idx: Option<usize> = None;

        for (i, attr) in variant.attrs.iter().enumerate() {
            if attr.path().is_ident("responsible") {
                match attr.parse_args::<Type>() {
                    Ok(ty) => {
                        found_ty = Some(ty);
                        found_idx = Some(i);
                    }
                    Err(e) => return e.to_compile_error().into(),
                }
                break;
            }
        }

        if let (Some(ty), Some(idx)) = (found_ty, found_idx) {
            variant.attrs.remove(idx);

            let sender_ty: Type = syn::parse2(quote! {
                ::tokio::sync::oneshot::Sender<#ty>
            })
            .unwrap();

            let sender_field = syn::Field {
                attrs: vec![],
                vis: syn::Visibility::Inherited,
                mutability: syn::FieldMutability::None,
                ident: None,
                colon_token: None,
                ty: sender_ty,
            };

            match &mut variant.fields {
                Fields::Unit => {
                    let mut unnamed = Punctuated::new();
                    unnamed.push(sender_field);
                    variant.fields = Fields::Unnamed(FieldsUnnamed {
                        paren_token: Paren(Span::call_site()),
                        unnamed,
                    });
                }
                Fields::Unnamed(fields) => {
                    fields.unnamed.push(sender_field);
                }
                Fields::Named(_) => {
                    return syn::Error::new_spanned(
                        &variant.ident,
                        "#[responsible] cannot be applied to a named-field variant",
                    )
                    .to_compile_error()
                    .into();
                }
            }
        }
    }

    quote! { #enum_def }.into()
}
