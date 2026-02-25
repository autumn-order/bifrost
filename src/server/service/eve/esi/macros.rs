//! Macros for defining ESI endpoints with automatic circuit breaker protection.
//!
//! This module provides macros that reduce boilerplate when defining ESI endpoint
//! methods by automatically wrapping them with circuit breaker logic.

/// Defines an ESI endpoint method that returns an `EsiProviderRequest`.
///
/// This macro generates a method that constructs an `EsiProviderRequest` wrapping
/// the underlying `EsiRequest` from the `eve_esi` crate. The returned request can
/// then be executed with either `.send()` or `.send_cached()`, both of which
/// automatically handle circuit breaker logic.
///
/// # Syntax
///
/// ```ignore
/// define_esi_endpoint! {
///     /// Documentation for the endpoint method.
///     ///
///     /// # Arguments
///     /// - `arg1` - Description
///     ///
///     /// # Returns
///     /// Description of return value
///     pub fn method_name(
///         &self,
///         arg1: Type1,
///         arg2: Type2,
///     ) -> EsiProviderRequest<ReturnType>
///     =>
///     category, endpoint_method[arg1, arg2]
/// }
/// ```
///
/// The `category, endpoint_method[args]` syntax automatically expands to:
/// `self.esi_client.category().endpoint_method(args)` and wraps it in an
/// `EsiProviderRequest` with the endpoint group's circuit breaker.
///
/// # Example
///
/// ```ignore
/// impl<'a> CharacterEndpoints<'a> {
///     define_esi_endpoint! {
///         /// Retrieves public information for a character.
///         pub fn get_character_public_information(
///             &self,
///             character_id: i64,
///         ) -> EsiProviderRequest<Character>
///         =>
///         character, get_character_public_information[character_id]
///     }
/// }
///
/// // Usage:
/// let request = esi_provider
///     .character()
///     .get_character_public_information(123456789);
///
/// // Execute with standard send
/// let response = request.send().await?;
///
/// // Or with cached send
/// let cached_response = request.send_cached(CacheStrategy::IfModifiedSince(timestamp)).await?;
/// ```
///
/// # Generated Code
///
/// The macro expands to a method that:
/// 1. Constructs the underlying `EsiRequest` from `eve_esi`
/// 2. Wraps it in an `EsiProviderRequest` with the endpoint group reference
/// 3. Returns the request for the caller to execute
#[macro_export]
macro_rules! define_esi_endpoint {
    (
        $(#[$meta:meta])*
        $vis:vis fn $name:ident(
            &self
            $(, $arg:ident: $arg_ty:ty)*
            $(,)?
        ) -> EsiProviderRequest<$ret:ty>
        =>
        $category:ident, $method:ident [$($call_arg:expr),* $(,)?]
    ) => {
        $(#[$meta])*
        ///
        /// # Returns
        /// `EsiProviderRequest` that can be executed with:
        /// - `.send()` - For fresh requests expecting 200 OK with data
        /// - `.send_cached(strategy)` - For conditional requests that may return 304 Not Modified
        ///
        /// # Errors
        /// - `AppError::EsiEndpointOffline` - Circuit breaker is open (endpoint offline)
        /// - `AppError::Esi` - ESI request failed (4xx/5xx errors, network issues, etc.)
        $vis fn $name(
            &self
            $(, $arg: $arg_ty)*
        ) -> $crate::server::service::eve::esi::request::EsiProviderRequest<'a, $ret> {
            let esi_request = self
                .esi_client
                .$category()
                .$method($($call_arg),*);

            $crate::server::service::eve::esi::request::EsiProviderRequest::new(
                self.group,
                esi_request,
            )
        }
    };
}

/// Re-export for internal use.
///
/// This allows endpoint modules to use `define_esi_endpoint!` without prefixing,
/// while the macro itself uses `$crate::` to reference types from this crate.
pub(super) use define_esi_endpoint;
