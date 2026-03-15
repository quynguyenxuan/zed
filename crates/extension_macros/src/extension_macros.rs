use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    Error, FnArg, GenericArgument, Ident, ImplItem, ImplItemFn, ItemImpl, LitStr,
    PathArguments, ReturnType, Token, Type,
    parse::{Parse, ParseStream, Parser},
    parse2,
    punctuated::Punctuated,
};

// ─── #[query_dispatch] / #[query_handler] ────────────────────────────────────

/// Applied to an `impl` block to generate an `on_query` dispatch method.
///
/// Mark individual methods with `#[query_handler("topic")]`. The macro
/// collects them, strips the attribute, and emits:
///
/// ```rust
/// fn on_query(&mut self, _query_id: u64, topic: String, _source: String, data: String)
///     -> Result<String, String>
/// {
///     match topic.as_str() {
///         "git.status" => zed_extension_api::query::respond::<Req, Res, _>(&data, |req| self.method(req)),
///         _ => Err(format!("no handler for topic: {}", topic)),
///     }
/// }
/// ```
///
/// Each annotated method must have the signature:
/// `fn name(&mut self, req: ReqType) -> Result<ResType, String>`
#[proc_macro_attribute]
pub fn query_dispatch(_attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_query_dispatch(item).unwrap_or_else(|e| e.to_compile_error().into())
}

/// Marks a method inside `#[query_dispatch]` as a handler for the given topic.
/// This attribute is stripped by `#[query_dispatch]` and must not be used standalone.
#[proc_macro_attribute]
pub fn query_handler(_attr: TokenStream, item: TokenStream) -> TokenStream {
    item
}

fn expand_query_dispatch(item: TokenStream) -> syn::Result<TokenStream> {
    let mut impl_block: ItemImpl = syn::parse(item)?;

    struct HandlerInfo {
        topic: String,
        method_ident: Ident,
        req_ty: Type,
        res_ty: Type,
    }

    let mut handlers = Vec::<HandlerInfo>::new();

    // First pass: strip #[query_handler] attrs and collect handler info.
    for item in &mut impl_block.items {
        let ImplItem::Fn(method) = item else {
            continue;
        };

        let mut found_topic: Option<String> = None;
        method.attrs.retain(|attr| {
            if attr.path().is_ident("query_handler") {
                if let Ok(lit) = attr.parse_args::<LitStr>() {
                    found_topic = Some(lit.value());
                }
                return false;
            }
            true
        });

        let Some(topic) = found_topic else { continue };

        let req_ty = req_type_of(method)?;
        let res_ty = res_type_of(method)?;

        handlers.push(HandlerInfo {
            topic,
            method_ident: method.sig.ident.clone(),
            req_ty,
            res_ty,
        });
    }

    if handlers.is_empty() {
        return Ok(quote!(#impl_block).into());
    }

    // Second pass: remove handler methods from the trait impl and collect them
    // for an inherent impl block (they are not trait methods).
    let handler_names: std::collections::HashSet<String> =
        handlers.iter().map(|h| h.method_ident.to_string()).collect();
    let mut handler_methods: Vec<ImplItemFn> = Vec::new();
    impl_block.items.retain(|item| {
        if let ImplItem::Fn(method) = item {
            if handler_names.contains(&method.sig.ident.to_string()) {
                handler_methods.push(method.clone());
                return false;
            }
        }
        true
    });

    let arms = handlers.iter().map(|h| {
        let topic = &h.topic;
        let method = &h.method_ident;
        let req_ty = &h.req_ty;
        let res_ty = &h.res_ty;
        quote! {
            #topic => ::zed_extension_api::query::respond::<#req_ty, #res_ty, _>(
                &data, |req| self.#method(req)
            ),
        }
    });

    let on_query_fn: ImplItemFn = parse2(quote! {
        fn on_query(
            &mut self,
            _query_id: u64,
            topic: String,
            _source: String,
            data: String,
        ) -> Result<String, String> {
            match topic.as_str() {
                #(#arms)*
                _ => Err(format!("no handler for topic: {}", topic)),
            }
        }
    })?;

    impl_block.items.push(ImplItem::Fn(on_query_fn));

    // Emit handler methods in a separate inherent impl block.
    let self_ty = &impl_block.self_ty;
    Ok(quote! {
        #impl_block

        impl #self_ty {
            #(#handler_methods)*
        }
    }
    .into())
}

fn req_type_of(method: &ImplItemFn) -> syn::Result<Type> {
    for arg in &method.sig.inputs {
        if let FnArg::Typed(pat_type) = arg {
            return Ok(*pat_type.ty.clone());
        }
    }
    Err(Error::new_spanned(
        &method.sig.ident,
        "#[query_handler] method must have a request parameter after &mut self",
    ))
}

fn res_type_of(method: &ImplItemFn) -> syn::Result<Type> {
    let ReturnType::Type(_, ty) = &method.sig.output else {
        return Err(Error::new_spanned(
            &method.sig.ident,
            "#[query_handler] method must return Result<T, String>",
        ));
    };
    let Type::Path(tp) = ty.as_ref() else {
        return Err(Error::new_spanned(ty, "return type must be Result<T, String>"));
    };
    let last = tp
        .path
        .segments
        .last()
        .ok_or_else(|| Error::new_spanned(ty, "empty return type"))?;
    let PathArguments::AngleBracketed(args) = &last.arguments else {
        return Err(Error::new_spanned(ty, "Result must have type arguments"));
    };
    let first = args
        .args
        .first()
        .ok_or_else(|| Error::new_spanned(ty, "Result must have a success type"))?;
    let GenericArgument::Type(res_ty) = first else {
        return Err(Error::new_spanned(first, "first type argument must be a type"));
    };
    Ok(res_ty.clone())
}

// ─── #[host_data(...)] ────────────────────────────────────────────────────────

struct HostDataField {
    name: Ident,
    ty: Type,
}

impl Parse for HostDataField {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![:]>()?;
        let ty: Type = input.parse()?;
        Ok(HostDataField { name, ty })
    }
}

/// Applied to `impl gui::Host for WasmState` inside `extension_host`.
///
/// Takes a comma-separated list of `field: Type` pairs as the attribute
/// argument. For each field it:
///
/// 1. Adds `async fn get_<field>(&mut self) -> wasmtime::Result<Type>` to
///    the annotated `impl gui::Host for WasmState` block.
/// 2. Generates `pub fn set_<field>(&mut self, value: Type)` in a new
///    `impl WasmState` block (used by the inject methods and tests).
/// 3. Generates `pub async fn inject_<field>(&self, value: Type) -> anyhow::Result<()>`
///    in a new `impl WasmExtension` block.
///
/// The fields themselves must still be declared manually on `WasmState`.
///
/// # Example
/// ```rust
/// #[host_data(active_file: Option<String>, project_root: Option<String>)]
/// impl gui::Host for WasmState {
///     async fn create_focus_handle(&mut self) -> wasmtime::Result<u32> { ... }
///     // get_active_file and get_project_root are generated
/// }
/// ```
#[proc_macro_attribute]
pub fn host_data(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_host_data(attr, item).unwrap_or_else(|e| e.to_compile_error().into())
}

fn expand_host_data(attr: TokenStream, item: TokenStream) -> syn::Result<TokenStream> {
    let fields =
        Punctuated::<HostDataField, Token![,]>::parse_terminated.parse(attr)?;

    let mut impl_block: ItemImpl = syn::parse(item)?;

    // 1. Add get_<name> methods to the impl block.
    for field in &fields {
        let name = &field.name;
        let ty = &field.ty;
        let getter_name = format_ident!("get_{}", name);
        let getter: ImplItemFn = parse2(quote! {
            async fn #getter_name(&mut self) -> wasmtime::Result<#ty> {
                Ok(self.#name.clone())
            }
        })?;
        impl_block.items.push(ImplItem::Fn(getter));
    }

    // 2. Generate set_<name> methods on WasmState.
    let setters: Vec<TokenStream2> = fields
        .iter()
        .map(|f| {
            let name = &f.name;
            let ty = &f.ty;
            let setter_name = format_ident!("set_{}", name);
            quote! {
                pub fn #setter_name(&mut self, value: #ty) {
                    self.#name = value;
                }
            }
        })
        .collect();

    // 3. Generate inject_<name> methods on WasmExtension.
    let injectors: Vec<TokenStream2> = fields
        .iter()
        .map(|f| {
            let name = &f.name;
            let ty = &f.ty;
            let inject_name = format_ident!("inject_{}", name);
            let setter_name = format_ident!("set_{}", name);
            quote! {
                pub async fn #inject_name(&self, value: #ty) -> anyhow::Result<()> {
                    self.call(move |_ext, store| {
                        ::futures::FutureExt::boxed(async move {
                            store.data_mut().#setter_name(value);
                        })
                    })
                    .await
                }
            }
        })
        .collect();

    let output = quote! {
        #impl_block

        impl WasmState {
            #(#setters)*
        }

        impl WasmExtension {
            #(#injectors)*
        }
    };

    Ok(output.into())
}
