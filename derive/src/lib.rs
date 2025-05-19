use proc_macro::TokenStream;
use quote::{ToTokens, quote};
use syn::{
	DeriveInput, Lit, LitStr, Meta, Path, parse_macro_input, punctuated::Punctuated, token::Comma,
};

#[proc_macro_derive(Service)]
pub fn derive_service(input: TokenStream) -> TokenStream {
	// Parse the input struct
	let input = parse_macro_input!(input as DeriveInput);
	let struct_name = &input.ident;

	// Collect args from #[service(...)]
	let mut svc_name: Option<String> = None;
	let mut fd_path: Option<Path> = None;

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
		} else {
			continue;
		}
	}

	let svc_name = svc_name.unwrap_or(struct_name.to_string());
	let fd_path = fd_path
		.map(|fd| fd.to_token_stream())
		.unwrap_or_else(|| quote! { &[] });

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
		}
	};

	TokenStream::from(expanded)
}
