extern crate proc_macro;

use quote::{format_ident, quote};
use syn::{parse_macro_input, ItemFn, Attribute, Token, FnArg};
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
    let attr_args = parse_macro_input!(args as FunctionToolAttribute);
    let input_fn = parse_macro_input!(item as ItemFn);
    
    let origin_ident = input_fn.sig.ident.clone();

    let mut function_description = attr_args
        .description.as_ref().cloned()
        .unwrap_or(String::new());
    
    let mut function_ident = attr_args
        .name.as_ref().cloned()
        .map(|e| syn::parse_str::<syn::Ident>(&e).unwrap())
        .unwrap_or(input_fn.sig.ident.clone());

    let parameters_struct_ident = format_ident!("{}Parameters", function_ident);
    let params = input_fn.sig.inputs
        .iter()
        .filter_map(|arg| {
            match arg {
                FnArg::Receiver(_) => None,
                FnArg::Typed(arg) => Some((arg.pat.clone(), arg.ty.clone())),
            }
        })
        .collect::<Vec<_>>();

    let parameter_fields = params
        .iter()
        .map(|(pat, ty)| quote! {
            #pat: #ty
        })
        .collect::<Vec<_>>();

    let arg_list = params.iter().map(|(pat, _)| {
        quote! { params.#pat }
    });
    
    let tool_struct_ident = format_ident!("{}Tool", function_ident);
    
    let parameter_struct = quote! {
        struct #tool_struct_ident {}
        
        #[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
        struct #parameters_struct_ident {
            #(#parameter_fields),*
        }

        impl_tool_params!(#parameters_struct_ident);

        #input_fn
    };

    let struct_impl = quote! {
        impl Tool for #tool_struct_ident {
            fn metadata(&self) -> ToolMetaData {
                ToolMetaData {
                    name: stringify!(#function_ident).to_string(),
                    description: stringify!(#function_description).to_string(),
                    parameters: #parameters_struct_ident :: schema(),
                }
            }

            fn execute(&self, parameters: Value) -> anyhow::Result<Value> {
                let params = serde_json::from_value::<#parameters_struct_ident>(parameters)?;
                let result = #origin_ident(#(#arg_list),*);
                Ok(serde_json::json! ({
                    "result": result,
                }))
            }
        }
    };


    quote! {
        #parameter_struct
        #struct_impl
    }.into()
}
