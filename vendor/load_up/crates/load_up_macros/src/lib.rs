mod load_asset_resource;

use proc_macro::TokenStream;

#[proc_macro_derive(LoadAssetResource, attributes(dependency))]
pub fn derive_load_asset_resource(input: TokenStream) -> TokenStream {
    load_asset_resource::derive(input)
}
