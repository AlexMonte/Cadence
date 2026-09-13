use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, spanned::Spanned, Data, DeriveInput, Error, ExprPath, Field, Fields,
    GenericArgument, LitFloat, LitInt, LitStr, PathArguments, Type,
};

use darling::FromMeta;

pub fn derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    match expand(input) {
        Ok(tokens) => tokens.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn expand(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let struct_ident = input.ident;

    let fields = match input.data {
        Data::Struct(data) => match data.fields {
            Fields::Named(fields) => fields.named,
            _ => {
                return Err(Error::new(
                    struct_ident.span(),
                    "LoadAssetResource only supports structs with named fields",
                ));
            }
        },
        _ => {
            return Err(Error::new(
                struct_ident.span(),
                "LoadAssetResource can only be derived for structs",
            ));
        }
    };

    let mut initializers = Vec::new();
    let mut visitors = Vec::new();
    let mut dependency_asset_types = Vec::new();

    for field in fields.iter() {
        let field_ident = field
            .ident
            .clone()
            .ok_or_else(|| Error::new(field.span(), "expected named field"))?;

        match parse_dependency_attr(field)? {
            DependencyAttr::Load(load) => {
                let field_name = field_ident.to_string();
                let initializer = load.initializer.clone();
                let role = load.role.tokens();
                let policy = load.policy.tokens();
                let threshold = load.threshold.tokens();
                let immediate = load.immediate;
                let primary_path = load
                    .primary_path
                    .as_ref()
                    .map(|path| quote!(Some(#path)))
                    .unwrap_or_else(|| quote!(None));
                let timeout = load
                    .timeout
                    .as_ref()
                    .map(|timeout| quote!(Some(std::time::Duration::from_secs_f64(#timeout))))
                    .unwrap_or_else(|| quote!(None));
                let acquire_fn_ident = format_ident!(
                    "__load_up_acquire_{}",
                    field_ident,
                    span = field_ident.span()
                );
                let fallback_fn_ident = format_ident!(
                    "__load_up_fallback_{}",
                    field_ident,
                    span = field_ident.span()
                );
                let replace_fn_ident = format_ident!(
                    "__load_up_replace_{}",
                    field_ident,
                    span = field_ident.span()
                );
                let acquire_fn = load.acquire_fn.clone();
                let handle_inner = load.handle_inner.clone();

                dependency_asset_types.push(load.handle_inner.clone());

                initializers.push(quote! {
                    #field_ident: #initializer
                });

                visitors.push(quote! {
                    fn #acquire_fn_ident(world: &mut bevy::prelude::World) -> bevy::prelude::UntypedHandle {
                        #acquire_fn
                    }

                    fn #fallback_fn_ident(
                        world: &mut bevy::prelude::World,
                        path: &'static str,
                    ) -> bevy::prelude::UntypedHandle {
                        world.resource::<bevy::prelude::AssetServer>()
                            .load::<#handle_inner>(path)
                            .untyped()
                    }

                    fn #replace_fn_ident(
                        world: &mut bevy::prelude::World,
                        prepared: &bevy::prelude::UntypedHandle,
                        replacement: bevy::prelude::UntypedHandle,
                    ) {
                        let replacement = replacement.typed::<#handle_inner>();
                        if let Some(mut value) = world
                            .resource_mut::<bevy::prelude::Assets<#struct_ident>>()
                            .get_mut(prepared.id().typed::<#struct_ident>())
                        {
                            value.#field_ident = replacement.clone();
                        }
                        if let Some(mut value) = world.get_resource_mut::<#struct_ident>() {
                            value.#field_ident = replacement;
                        }
                    }

                    f(load_up::dependency::LoadUpDependency {
                        field: #field_name,
                        handle: self.#field_ident.clone().untyped(),
                        role: #role,
                        policy: #policy,
                        state_safe_threshold: #threshold,
                        acquire: #acquire_fn_ident,
                        acquire_fallback: #fallback_fn_ident,
                        replace: #replace_fn_ident,
                        primary_path: #primary_path,
                        timeout: #timeout,
                        immediate: #immediate,
                    });
                });
            }
            DependencyAttr::Default => {
                initializers.push(quote! {
                    #field_ident: Default::default()
                });
            }
            DependencyAttr::Skip => {
                return Err(Error::new(
                    field.span(),
                    "#[dependency(skip)] is not supported. Use #[dependency(default)], #[dependency(init = ...)], or #[dependency(path = ...)] instead.",
                ));
            }
            DependencyAttr::Missing => {
                return Err(Error::new(
                    field.span(),
                    "missing #[dependency(...)] attribute. Use #[dependency(path = ...)], #[dependency(init = ...)], or #[dependency(default)].",
                ));
            }
        }
    }

    let asset_inits = dependency_asset_types.iter().map(|handle_inner| {
        quote! {
            if !app.world().contains_resource::<bevy::prelude::Assets<#handle_inner>>() {
                app.init_asset::<#handle_inner>();
            }
        }
    });

    let name = struct_ident.to_string();

    Ok(quote! {
        impl bevy::prelude::FromWorld for #struct_ident {
            fn from_world(world: &mut bevy::prelude::World) -> Self {
                Self {
                    #(#initializers,)*
                }
            }
        }

        impl load_up::prepared::LoadAssetResource for #struct_ident {
            const NAME: &'static str = #name;

            fn register_dependency_assets(app: &mut bevy::prelude::App) {
                #(#asset_inits)*
            }

            fn visit_load_up_dependencies(
                &self,
                f: &mut dyn FnMut(load_up::dependency::LoadUpDependency),
            ) {
                #(#visitors)*
            }
        }
    })
}

enum DependencyAttr {
    Load(Box<LoadDependencyAttr>),
    Default,
    Skip,
    Missing,
}

struct LoadDependencyAttr {
    initializer: proc_macro2::TokenStream,
    acquire_fn: proc_macro2::TokenStream,
    handle_inner: Type,
    role: DependencyRoleAttr,
    policy: DependencyPolicyAttr,
    threshold: ThresholdAttr,
    primary_path: Option<LitStr>,
    timeout: Option<LitFloat>,
    immediate: bool,
}

#[derive(Clone, Copy)]
enum DependencyRoleAttr {
    Blocking,
    Streaming,
}

impl DependencyRoleAttr {
    fn tokens(self) -> proc_macro2::TokenStream {
        match self {
            Self::Blocking => quote!(load_up::dependency::DependencyRole::Blocking),
            Self::Streaming => quote!(load_up::dependency::DependencyRole::Streaming),
        }
    }
}

enum DependencyPolicyAttr {
    Fatal,
    Ignore,
    Retry {
        attempts: LitInt,
        cooldown_seconds: proc_macro2::TokenStream,
    },
    Fallback(LitStr),
}

impl DependencyPolicyAttr {
    fn tokens(&self) -> proc_macro2::TokenStream {
        match self {
            Self::Fatal => quote!(load_up::dependency::FailurePolicy::Fatal),
            Self::Ignore => quote!(load_up::dependency::FailurePolicy::Ignore),
            Self::Retry {
                attempts,
                cooldown_seconds,
            } => quote!(load_up::dependency::FailurePolicy::Retry {
                attempts: #attempts,
                cooldown: std::time::Duration::from_secs_f64(#cooldown_seconds)
            }),
            Self::Fallback(path) => {
                quote!(load_up::dependency::FailurePolicy::Fallback { path: #path })
            }
        }
    }
}

#[derive(Clone, Copy)]
enum ThresholdAttr {
    Ready,
    Loaded,
}

impl ThresholdAttr {
    fn tokens(self) -> proc_macro2::TokenStream {
        match self {
            Self::Ready => quote!(load_up::dependency::StateSafeThreshold::Ready),
            Self::Loaded => quote!(load_up::dependency::StateSafeThreshold::Loaded),
        }
    }
}

#[derive(Default, FromMeta)]
#[darling(default)]
struct DependencyOptions {
    path: Option<String>,
    init: Option<syn::Path>,
    blocking: bool,
    streaming: bool,
    default: bool,
    skip: bool,
    on_fail: Option<syn::Ident>,
    fallback: Option<String>,
    retry: Option<LitInt>,
    retry_cooldown: Option<LitFloat>,
    timeout: Option<LitFloat>,
    state_safe: Option<syn::Ident>,
}

fn parse_dependency_attr(field: &Field) -> syn::Result<DependencyAttr> {
    let Some(attr) = field
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("dependency"))
    else {
        return Ok(DependencyAttr::Missing);
    };

    let options = DependencyOptions::from_meta(&attr.meta)
        .map_err(|error| Error::new(attr.span(), error.to_string()))?;

    if options.blocking && options.streaming {
        return Err(Error::new(
            attr.span(),
            "a dependency cannot be both `blocking` and `streaming`",
        ));
    }
    let failure_directives = usize::from(options.retry.is_some())
        + usize::from(options.fallback.is_some())
        + usize::from(options.on_fail.is_some());
    if failure_directives > 1 {
        return Err(Error::new(
            attr.span(),
            "retry, fallback, and on_fail directives are mutually exclusive",
        ));
    }
    if options.retry_cooldown.is_some() && options.retry.is_none() {
        return Err(Error::new(attr.span(), "`retry_cooldown` requires `retry`"));
    }
    if options
        .retry
        .as_ref()
        .is_some_and(|attempts| attempts.base10_parse::<u8>().ok() == Some(0))
    {
        return Err(Error::new(
            attr.span(),
            "`retry` attempts must be greater than zero",
        ));
    }
    if options
        .retry
        .as_ref()
        .is_some_and(|attempts| attempts.base10_parse::<u8>().is_err())
    {
        return Err(Error::new(attr.span(), "`retry` must fit in a u8"));
    }
    if options.retry_cooldown.as_ref().is_some_and(|cooldown| {
        cooldown
            .base10_parse::<f64>()
            .map_or(true, |value| !value.is_finite() || value < 0.0)
    }) {
        return Err(Error::new(
            attr.span(),
            "`retry_cooldown` must be finite and non-negative",
        ));
    }
    if options.timeout.as_ref().is_some_and(|timeout| {
        timeout
            .base10_parse::<f64>()
            .map_or(true, |value| !value.is_finite() || value <= 0.0)
    }) {
        return Err(Error::new(
            attr.span(),
            "`timeout` must be finite and greater than zero",
        ));
    }
    if options.init.is_some() && options.fallback.is_some() {
        return Err(Error::new(
            attr.span(),
            "`fallback` is only supported with a typed `path` dependency",
        ));
    }
    if options.init.is_some() && options.timeout.is_some() {
        return Err(Error::new(
            attr.span(),
            "`timeout` is not supported for immediate `init` dependencies",
        ));
    }

    let path = options.path.map(|value| LitStr::new(&value, attr.span()));
    let primary_path = path.clone();
    let timeout = options.timeout;
    let init = options.init.map(|path| ExprPath {
        attrs: Vec::new(),
        qself: None,
        path,
    });

    let role = if options.streaming {
        DependencyRoleAttr::Streaming
    } else {
        DependencyRoleAttr::Blocking
    };

    let policy = if let Some(attempts) = options.retry {
        let cooldown_seconds = options
            .retry_cooldown
            .map(|cooldown| quote!(#cooldown))
            .unwrap_or_else(|| quote!(0.0f64));
        Some(DependencyPolicyAttr::Retry {
            attempts,
            cooldown_seconds,
        })
    } else if let Some(fallback) = options.fallback {
        Some(DependencyPolicyAttr::Fallback(LitStr::new(
            &fallback,
            attr.span(),
        )))
    } else if let Some(mode) = &options.on_fail {
        Some(match mode.to_string().as_str() {
            "fatal" => DependencyPolicyAttr::Fatal,
            "ignore" => DependencyPolicyAttr::Ignore,
            other => {
                return Err(Error::new(
                    attr.span(),
                    format!("unsupported on_fail mode `{other}`; expected `fatal` or `ignore`"),
                ));
            }
        })
    } else {
        None
    };

    let threshold = match options
        .state_safe
        .as_ref()
        .map(syn::Ident::to_string)
        .as_deref()
    {
        None | Some("ready") => ThresholdAttr::Ready,
        Some("loaded") => ThresholdAttr::Loaded,
        Some(other) => {
            return Err(Error::new(
                attr.span(),
                format!("unsupported state_safe mode `{other}`; expected `ready` or `loaded`"),
            ));
        }
    };

    if options.default {
        if path.is_some()
            || init.is_some()
            || policy.is_some()
            || timeout.is_some()
            || matches!(role, DependencyRoleAttr::Streaming)
        {
            return Err(Error::new(
                attr.span(),
                "#[dependency(default)] cannot be combined with path, init, role, or failure policy directives",
            ));
        }
        return Ok(DependencyAttr::Default);
    }

    if options.skip {
        if path.is_some()
            || init.is_some()
            || policy.is_some()
            || timeout.is_some()
            || matches!(role, DependencyRoleAttr::Streaming)
        {
            return Err(Error::new(
                attr.span(),
                "#[dependency(skip)] cannot be combined with path, init, role, or failure policy directives",
            ));
        }
        return Ok(DependencyAttr::Skip);
    }

    if path.is_some() && init.is_some() {
        return Err(Error::new(
            attr.span(),
            "#[dependency(...)] cannot specify both `path` and `init`",
        ));
    }

    let handle_inner = extract_handle_inner_type(&field.ty).ok_or_else(|| {
        Error::new(
            field.ty.span(),
            "#[dependency(...)] can only be used on Handle<T> fields",
        )
    })?;

    let (initializer, acquire_fn, immediate) = if let Some(path) = path {
        (
            quote!({
                let asset_server = world.resource::<bevy::prelude::AssetServer>();
                asset_server.load::<#handle_inner>(#path)
            }),
            quote!({
                let asset_server = world.resource::<bevy::prelude::AssetServer>();
                asset_server.load::<#handle_inner>(#path).untyped()
            }),
            false,
        )
    } else if let Some(init_fn) = init {
        (
            quote!({
                let mut assets = world.resource_mut::<bevy::prelude::Assets<#handle_inner>>();
                assets.add(#init_fn())
            }),
            quote!({
                let mut assets = world.resource_mut::<bevy::prelude::Assets<#handle_inner>>();
                assets.add(#init_fn()).untyped()
            }),
            true,
        )
    } else {
        return Ok(DependencyAttr::Missing);
    };

    let policy = policy.unwrap_or(match role {
        DependencyRoleAttr::Blocking => DependencyPolicyAttr::Fatal,
        DependencyRoleAttr::Streaming => DependencyPolicyAttr::Ignore,
    });

    Ok(DependencyAttr::Load(Box::new(LoadDependencyAttr {
        initializer,
        acquire_fn,
        handle_inner,
        role,
        policy,
        threshold,
        primary_path,
        timeout,
        immediate,
    })))
}

fn extract_handle_inner_type(ty: &Type) -> Option<Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };

    let last = type_path.path.segments.last()?;

    if last.ident != "Handle" {
        return None;
    }

    let PathArguments::AngleBracketed(args) = &last.arguments else {
        return None;
    };

    let first = args.args.first()?;

    let GenericArgument::Type(inner) = first else {
        return None;
    };

    Some(inner.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse::Parser;

    fn parse_field(input: proc_macro2::TokenStream) -> Field {
        Field::parse_named
            .parse2(input)
            .expect("field should parse")
    }

    #[test]
    fn rejects_path_and_init_together() {
        let field = parse_field(quote! {
            #[dependency(path = "a.png", init = make_asset)]
            asset: Handle<Image>
        });

        let error = parse_dependency_attr(&field)
            .err()
            .expect("mixed acquisition should fail");
        assert!(error.to_string().contains("both `path` and `init`"));
    }

    #[test]
    fn rejects_default_mixed_with_path() {
        let field = parse_field(quote! {
            #[dependency(default, path = "a.png")]
            asset: Handle<Image>
        });

        let error = parse_dependency_attr(&field)
            .err()
            .expect("default mix should fail");
        assert!(error.to_string().contains("cannot be combined"));
    }

    #[test]
    fn parses_retry_with_default_cooldown() {
        let field = parse_field(quote! {
            #[dependency(path = "a.png", retry = 3)]
            asset: Handle<Image>
        });

        let attr = parse_dependency_attr(&field).expect("retry literal should parse");
        let DependencyAttr::Load(load) = attr else {
            panic!("expected load attribute");
        };

        match load.policy {
            DependencyPolicyAttr::Retry {
                attempts,
                cooldown_seconds,
            } => {
                assert_eq!(attempts.base10_digits(), "3");
                assert_eq!(cooldown_seconds.to_string(), "0.0f64");
            }
            _ => panic!("expected retry policy"),
        }
    }

    #[test]
    fn parses_retry_cooldown_and_state_safe_loaded() {
        let field = parse_field(quote! {
            #[dependency(path = "a.png", retry = 2, retry_cooldown = 1.5, state_safe = loaded)]
            asset: Handle<Image>
        });

        let attr = parse_dependency_attr(&field).expect("retry cooldown should parse");
        let DependencyAttr::Load(load) = attr else {
            panic!("expected load attribute");
        };

        match load.policy {
            DependencyPolicyAttr::Retry {
                attempts,
                cooldown_seconds,
            } => {
                assert_eq!(attempts.base10_digits(), "2");
                assert_eq!(cooldown_seconds.to_string(), "1.5");
            }
            _ => panic!("expected retry policy"),
        }

        assert!(matches!(load.threshold, ThresholdAttr::Loaded));
    }
}
