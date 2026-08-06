#![forbid(unsafe_code)]

extern crate proc_macro;

use proc_macro::TokenStream;
use minify::{
    extract_template_path, get_minify_options_from_token_stream, minify_template,
    modify_template_path, replace_path_attribute, MinifyOptions,
};

mod minify;

#[proc_macro_derive(TemplateSimple, attributes(template, min_with))]
pub fn derive_template_simple(tokens: TokenStream) -> TokenStream {
    let token_str = tokens.to_string();

    let file_path = match extract_template_path(&token_str) {
        Ok(path) => path,
        Err(err) => return err.into_compile_error().into(),
    };
    let new_path = modify_template_path(&file_path);

    let mut minify_options = MinifyOptions::default();
    if let Err(err) =
        get_minify_options_from_token_stream(tokens.clone().into(), &mut minify_options)
    {
        return err.into_compile_error().into();
    }

    if let Err(err) = minify_template(&file_path, &new_path, &minify_options) {
        panic!(
            "sailfish-minify: failed to minify template {:?}: {}",
            file_path, err
        );
    }

    let input = replace_path_attribute(tokens.into(), new_path.to_str().unwrap());

    let output = sailfish_compiler::procmacro::derive_template_simple(input);

    TokenStream::from(output)
}
