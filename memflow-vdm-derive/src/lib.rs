use std::env;

use darling::FromDeriveInput;

use proc_macro::TokenStream;

use quote::quote;

use syn::{DeriveInput, parse_macro_input};

#[derive(FromDeriveInput)]
#[darling(attributes(connector), forward_attrs(allow, doc, cfg))]
struct ConnectorOpts {
    /// A unique identifier for the connector that allows memflow's plugin inventory system to
    /// locate and instantiate it.
    conn_name: String,

    /// When set to `true`, environment variables are used to override the `driver_path` and
    /// `service_name` values, respectively, if they exist. If they don't exist, the corresponding
    /// attribute value is used as a fallback.
    ///
    /// Environment variables follow the pattern `{CONN_NAME}_{SETTING}` where:
    /// - `{CONN_NAME}` is the connector name in uppercase.
    /// - `{SETTING}` is either `DRIVER_PATH` or `SERVICE_NAME`.
    ///
    /// For example, with `conn_name` set to "winio":
    /// - `WINIO_DRIVER_PATH` would override the `driver_path` value.
    /// - `WINIO_SERVICE_NAME` would override the `service_name` value.
    #[darling(default)]
    use_env_vars: bool,

    /// Optional name of the Windows service associated with the vulnerable driver.
    ///
    /// If provided, a service with this name will be created (if `driver_path` is also provided
    /// and no service with this name exists) or opened (if `driver_path` is `None`), assuming
    /// there's an existing service with this name.
    ///
    /// When `use_env_vars` is `true`, this value is overridden by the value from the
    /// `{CONN_NAME}_SERVICE_NAME` environment variable if it exists. If it doesn't exist, this
    /// value will be used as a fallback.
    service_name: Option<String>,

    /// Optional path to the vulnerable driver file (e.g., `C:\\winio64.sys`).
    ///
    /// If provided along with `service_name`, it will be used to create a new Windows service (if
    /// no service with the given name already exists).
    ///
    /// When `use_env_vars` is `true`, this value is overridden by the value from the
    /// `{CONN_NAME}_DRIVER_PATH` environment variable if it exists. If it doesn't exist, this
    /// value will be used as a fallback.
    driver_path: Option<String>,

    /// Path to the function used to create the driver instance returning `Result<Self>`.
    ///
    /// By default, this macro looks for an `open` function in your struct implementation. However,
    /// this attribute can be used to specify an alternative name if you've implemented the
    /// constructor function under a different name.
    ///
    /// Defaults to `open` if not specified.
    func: Option<syn::Path>,
}

#[proc_macro_derive(VdmDriver, attributes(connector))]
pub fn derive_vdm_driver(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let opts = match ConnectorOpts::from_derive_input(&input) {
        Ok(v) => v,
        Err(e) => return e.write_errors().into(),
    };

    let ident = &input.ident;

    let ConnectorOpts {
        conn_name,
        use_env_vars,
        driver_path,
        service_name,
        func,
    } = opts;

    let func = func.unwrap_or_else(|| syn::parse_str("open").unwrap());

    let builder = match service_name {
        Some(service_name) => {
            let service_name = if use_env_vars {
                let env_key = format!("{}_SERVICE_NAME", conn_name.to_uppercase());

                env::var(env_key).unwrap_or(service_name)
            } else {
                service_name
            };

            let driver_path = driver_path
                .map(|path| {
                    let path = if use_env_vars {
                        let env_key = format!("{}_DRIVER_PATH", conn_name.to_uppercase());

                        env::var(env_key).unwrap_or(path)
                    } else {
                        path
                    };

                    quote! { Some(#path) }
                })
                .unwrap_or(quote! { None });

            quote! {
                ::memflow_vdm::VdmConnectorBuilder::new()
                    .with_service(#service_name, #driver_path, || #ident::#func())?
            }
        }
        None => {
            quote! {
                 let drv = #ident::#func()?;

                ::memflow_vdm::VdmConnectorBuilder::new().with_memory(drv)
            }
        }
    };

    let output = quote! {
        #[::memflow::derive::connector(name = #conn_name)]
        pub fn create_connector<'a>(
            _args: &::memflow::plugins::connector::ConnectorArgs
        ) -> ::memflow::error::Result<::memflow_vdm::VdmConnector<'a, #ident>> {
            #builder
                .build()
                .map_err(Into::into)
        }
    };

    output.into()
}
