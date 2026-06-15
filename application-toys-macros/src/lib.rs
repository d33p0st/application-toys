extern crate proc_macro;

#[cfg(feature = "asynchronous-traits")]
mod async_macro;

#[cfg(feature = "event")]
mod responsible_macro;

#[cfg(feature = "asynchronous-traits")]
#[proc_macro_attribute]
pub fn asynchronous(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    async_macro::asynchronous_macro(attr, item)
}

#[cfg(feature = "event")]
#[proc_macro_attribute]
pub fn responsible(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    responsible_macro::responsible_macro(attr, item)
}
