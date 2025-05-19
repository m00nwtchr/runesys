use proc_macro::TokenStream;
use proc_macro2::Ident;
use quote::{ToTokens, quote};
use syn::{DeriveInput, LitStr, Path, parse_macro_input};

#[proc_macro_derive(Service, attributes(service, server, fd_set))]
pub fn derive_service(input: TokenStream) -> TokenStream {
	// Parse the input struct
	let input = parse_macro_input!(input as DeriveInput);
	let struct_name = &input.ident;

	// Collect args from #[service(...)]
	let mut svc_name: Option<String> = None;
	let mut fd_path: Option<Path> = None;
	let mut server: Option<Ident> = None;

	for attr in input.attrs {
		if attr.path().is_ident("service") {
			let str: LitStr = match attr.parse_args() {
				Ok(str) => str,
				Err(err) => {
					return err.into_compile_error().into();
				}
			};

			svc_name = Some(str.value());
		} else if attr.path().is_ident("fd_set") {
			let path: Path = match attr.parse_args() {
				Ok(path) => path,
				Err(err) => {
					return err.into_compile_error().into();
				}
			};

			fd_path = Some(path);
		} else if attr.path().is_ident("server") {
			let path: Ident = match attr.parse_args() {
				Ok(path) => path,
				Err(err) => {
					return err.into_compile_error().into();
				}
			};

			server = Some(path);
		} else {
			continue;
		}
	}

	let svc_name = svc_name.unwrap_or(struct_name.to_string());
	let fd_path = fd_path
		.map(|fd| fd.to_token_stream())
		.unwrap_or_else(|| quote! { &[] });
	let server = server.expect("Missing #[server(...)] attribute");

	// Generate the impl block
	let expanded = quote! {
		impl ::runesys::Service for #struct_name {
			const INFO: ::runesys::ServiceInfo = ::runesys::ServiceInfo {
				name: #svc_name,
				pkg: env!("CARGO_PKG_NAME"),
				version: env!("CARGO_PKG_VERSION"),
			};

			#[cfg(debug_assertions)]
			const FILE_DESCRIPTOR_SET: &'static [u8] = #fd_path;

			type Server = #server<Self>;
			fn new_server(self) -> Self::Server {
				Self::Server::new(self)
			}
		}
	};

	TokenStream::from(expanded)
}
