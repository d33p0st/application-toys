use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::{
    parse::Parse, parse::ParseStream, FnArg, GenericParam, ImplItem, ItemImpl, ItemTrait,
    Lifetime, LifetimeParam, ReturnType, Token, TraitItem,
};

struct Args {
    /// Remove the `Sync` bound (keep only `Send`).
    no_sync: bool,
    /// Remove both `Send` and `Sync` bounds (single-threaded code).
    local: bool,
    /// Force `'static` lifetime even when `&self`/`&mut self` is present.
    static_lifetime: bool,
}

impl Parse for Args {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut no_sync = false;
        let mut local = false;
        let mut static_lifetime = false;

        while !input.is_empty() {
            let ident: syn::Ident = input.parse()?;
            match ident.to_string().as_str() {
                "no_sync" => no_sync = true,
                "local" => local = true,
                "static_lifetime" => static_lifetime = true,
                other => {
                    return Err(syn::Error::new(
                        ident.span(),
                        format!(
                            "unknown argument `{other}`; expected `no_sync`, `local`, or `static_lifetime`"
                        ),
                    ))
                }
            }
            if !input.is_empty() {
                input.parse::<Token![,]>()?;
            }
        }

        Ok(Args { no_sync, local, static_lifetime })
    }
}

pub fn asynchronous_macro(attr: TokenStream, item: TokenStream) -> TokenStream {
    let args = syn::parse_macro_input!(attr as Args);
    let item_clone = item.clone();

    if let Ok(trait_def) = syn::parse::<ItemTrait>(item) {
        return transform_trait(trait_def, &args).into();
    }

    if let Ok(impl_def) = syn::parse::<ItemImpl>(item_clone) {
        return transform_impl(impl_def, &args).into();
    }

    syn::Error::new(
        Span::call_site(),
        "#[asynchronous] can only be applied to `trait` or `impl` blocks",
    )
    .to_compile_error()
    .into()
}

/// Returns true if the first argument is `&self` or `&mut self` (a borrowed receiver).
fn has_ref_receiver(sig: &syn::Signature) -> bool {
    matches!(
        sig.inputs.first(),
        Some(FnArg::Receiver(r)) if r.reference.is_some()
    )
}

/// Builds the `+ Send + Sync` (or subset) bounds based on args.
fn thread_bounds(args: &Args) -> TokenStream2 {
    if args.local {
        return quote! {};
    }
    let sync = if args.no_sync {
        quote! {}
    } else {
        quote! { + ::core::marker::Sync }
    };
    quote! { + ::core::marker::Send #sync }
}

/// Produces the full `Pin<Box<dyn Future<...>>>` type tokens.
fn pinbox_future(output_ty: &TokenStream2, args: &Args, lt: Option<&Lifetime>) -> TokenStream2 {
    let bounds = thread_bounds(args);
    match lt {
        Some(lifetime) => quote! {
            ::std::pin::Pin<
                ::std::boxed::Box<
                    dyn ::std::future::Future<Output = #output_ty> #bounds + #lifetime
                >
            >
        },
        None => quote! {
            ::std::pin::Pin<
                ::std::boxed::Box<
                    dyn ::std::future::Future<Output = #output_ty> #bounds + 'static
                >
            >
        },
    }
}

/// Strips `async`, injects the lifetime generic + receiver annotation, and rewrites the return type.
/// Does NOT touch the block.
fn rewrite_sig(sig: &mut syn::Signature, args: &Args) -> Option<Lifetime> {
    if sig.asyncness.is_none() {
        return None;
    }
    sig.asyncness = None;

    let output_ty = match &sig.output {
        ReturnType::Default => quote! { () },
        ReturnType::Type(_, ty) => quote! { #ty },
    };

    let lt = if !args.static_lifetime && has_ref_receiver(sig) {
        Some(Lifetime::new("'async_trait", Span::call_site()))
    } else {
        None
    };

    if let Some(ref lifetime) = lt {
        // Prepend 'async_trait to the function's generic params
        sig.generics
            .params
            .insert(0, GenericParam::Lifetime(LifetimeParam::new(lifetime.clone())));

        // Annotate &self / &mut self with 'async_trait
        for arg in sig.inputs.iter_mut() {
            if let FnArg::Receiver(r) = arg {
                if let Some((_, lt_slot)) = r.reference.as_mut() {
                    *lt_slot = Some(lifetime.clone());
                }
            }
        }
    }

    let ret_ty = pinbox_future(&output_ty, args, lt.as_ref());
    sig.output = syn::parse2(quote! { -> #ret_ty }).unwrap();

    lt
}

fn transform_trait(mut trait_def: ItemTrait, args: &Args) -> TokenStream2 {
    for item in trait_def.items.iter_mut() {
        if let TraitItem::Fn(method) = item {
            if method.sig.asyncness.is_some() {
                rewrite_sig(&mut method.sig, args);
                if let Some(ref block) = method.default {
                    let stmts = block.stmts.clone();
                    method.default = Some(
                        syn::parse2(quote! {
                            {
                                ::std::boxed::Box::pin(async move {
                                    #(#stmts)*
                                })
                            }
                        })
                        .unwrap(),
                    );
                }
            }
        }
    }
    quote! { #trait_def }
}

fn transform_impl(mut impl_def: ItemImpl, args: &Args) -> TokenStream2 {
    for item in impl_def.items.iter_mut() {
        if let ImplItem::Fn(method) = item {
            if method.sig.asyncness.is_some() {
                rewrite_sig(&mut method.sig, args);
                // Wrap the original body in Box::pin(async move { ... })
                let stmts = method.block.stmts.clone();
                method.block = syn::parse2(quote! {
                    {
                        ::std::boxed::Box::pin(async move {
                            #(#stmts)*
                        })
                    }
                })
                .unwrap();
            }
        }
    }
    quote! { #impl_def }
}
