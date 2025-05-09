use darling::FromDeriveInput;

use proc_macro::TokenStream;

use quote::quote;

use syn::{DeriveInput, parse_macro_input};

#[derive(FromDeriveInput)]
#[darling(attributes(connector), forward_attrs(allow, doc, cfg))]
struct Opts {
    /// Unique identifier for the connector (required).
    conn_name: String,

    /// Optional name of the existing Windows service associated with the vulnerable driver.
    /// If specified, the service will be started automatically if not already running.
    service_name: Option<String>,

    /// Optional path to the function used to create the driver instance returning `Result<Self>`.
    /// Defaults to `open` if not specified.
    func: Option<syn::Path>,
}

#[proc_macro_derive(VdmDriver, attributes(connector))]
pub fn derive_vdm_driver(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let opts = match Opts::from_derive_input(&input) {
        Ok(v) => v,
        Err(e) => return TokenStream::from(e.write_errors()),
    };

    let ident = &input.ident;
    let conn_name = &opts.conn_name;
    let func = opts.func.unwrap_or_else(|| syn::parse_str("open").unwrap());

    let builder = match opts.service_name.as_ref() {
        Some(service_name) => {
            quote! {
                ::memflow_vdm::VdmConnectorBuilder::new()
                    .with_service(#service_name, || #ident::#func())?
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
