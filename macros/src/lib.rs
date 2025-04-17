extern crate proc_macro;

use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, Attribute, Token};
use syn::parse::{Parse, ParseStream};
use regex::Regex;

#[derive(Debug)]
struct FunctionToolAttribute {
    name: Option<String>,
    description: Option<String>,
}

impl Parse for FunctionToolAttribute {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut name = None;
        let mut description = None;

        let check_name_pattern = Regex::new(r"^[_a-zA-Z][_a-zA-Z0-9]*").unwrap();
        
        while !input.is_empty() {
            let key = input.parse::<syn::Ident>()?;
            let _eq = input.parse::<Token![=]>()?;
            let value = input.parse::<syn::LitStr>()?;

            match key.to_string().as_str() {
                "name" => {
                    if !check_name_pattern.is_match(&value.value()) {
                        return Err(syn::Error::new(key.span(), format!("Value {} isn't proper ident", &value.value())));
                    }
                    name = Some(value.value());
                }
                "description" => description = Some(value.value()),
                _ => return Err(syn::Error::new(key.span(), "expected `name`, `description`")),
            }

            if input.peek(Token![,]) {
                let _ = input.parse::<Token![,]>()?;
            }
        }

        Ok(FunctionToolAttribute { name, description })
    }
}

#[proc_macro_attribute]
pub fn function_tool(args: proc_macro::TokenStream, item: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let attr_args = parse_macro_input!(args with Attribute::parse_outer);
    let input_fn = parse_macro_input!(item as ItemFn);

    let mut function_ident = input_fn.sig.ident.clone();
    let mut function_description = String::from("");
    
    let attribute = input_fn.attrs.iter().find(|attr| {
        attr.path().is_ident("function_tool")
    });

    
    if let Some(attr) = attribute {
        match attr.parse_args::<FunctionToolAttribute>() {
            Ok(FunctionToolAttribute { name, description }) => {
                if let Some(name) = name {
                    function_ident = syn::parse_str::<syn::Ident>(&name).unwrap();
                }
                if let Some(inner_description) = description {
                    function_description = inner_description;
                }
            }
            Err(e) => return e.into_compile_error().into(),
        }
    }



    quote! {

    }.into()
}
