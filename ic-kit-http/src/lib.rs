//! This crate seeks to provide an easy to use framework for writing canisters that accept HTTP requests
//! on the Internet Computer.
//!
//!  For the router, we use [`matchit`], a *blazing* fast URL router.
//!
//! It is built in conjunction with the [ic_kit_macros] crate. The macros
//! provided by this crate are used to generate a canister method `http_request` and `http_request_update`
//! that routes the HTTP requests to handlers. A `Router` struct and implementation will be generated
//! and used that will dispatch the HTTP requests to the appropriate handler.
//!
//! ## Example
//!
//! For a complete example, see the [`pastebin`].
//!
//! ## Macro Generated Router
//! The macro generated router will have a field for each HTTP method, as well as a generic `insert`
//! and `at` method. These accept the same arguments as the `insert` and `at` methods of the [`BasicRouter`],
//! but with an additional argument for the HTTP method.
//! A router can look like this:
//! ```
//! use ic_kit_http::*;
//!
//! pub struct Router {
//!    pub get: BasicRouter<HandlerFn>,
//!    // if there are no handlers for the method, the router will not have a field or implementation for it
//!    pub post: BasicRouter<HandlerFn>,
//!    pub put: BasicRouter<HandlerFn>,
//!    pub delete: BasicRouter<HandlerFn>,
//! }
//!
//! impl Router {
//!     pub fn insert(&mut self, method: &str, path: &str, handler: HandlerFn) {
//!         match method {
//!             "get" => self.get.insert(path, handler).unwrap(),
//!             "post" => self.post.insert(path, handler).unwrap(),
//!             "put" => self.put.insert(path, handler).unwrap(),
//!             "delete" => self.delete.insert(path, handler).unwrap(),
//!             _ => panic!("unsupported method: " + method),
//!         };
//!     }
//!     pub fn at<'s: 'p, 'p>(
//!         &'s self,
//!         method: &str,
//!         path: &'p str,
//!     ) -> Result<Match<'s, 'p, &HandlerFn>, MatchError> {
//!         match method {
//!             "get" => self.get.at(path),
//!             "post" => self.post.at(path),
//!             "put" => self.put.at(path),
//!             "delete" => self.delete.at(path),
//!             _ => Err(MatchError::NotFound),
//!         }
//!     }
//! }
//! ```
//!
//! ## Macro Generated http_request update and query calls
//! The macros will also generate a `http_request` query call handler. They will utilize
//! the generated router to dispatch the request to the appropriate handler. It also performs the
//! necessary path to upgrade to an additional update method (`http_request_update`) if the handler
//! is marked as upgraded.
//!
//! [`pastebin`]: https://github.com/Psychedelic/ic-kit/tree/main/examples/pastebin

use std::collections::HashMap;

use candid::{CandidType, Deserialize, Func, Nat};
/// The macro generated `Router` struct will return this type when a route is matched.
///
pub use matchit::Match;
pub use matchit::{MatchError, Params, Router as BasicRouter};

pub use ic_kit_macros::{delete, get, post, put};

/// Alias for a key/value header tuple
pub type HeaderField = (String, String);

/// # HttpRequest
/// The IC data type for a request.
/// * `method` - the HTTP method of the request.
/// * `url` - the URL of the request.
/// * `headers` - a list of key-value pairs.
/// * `body` - a raw byte array for the request body.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<HeaderField>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    /// Returns the value of the header with the given name. Case Insensitive.
    /// If the header is not present, returns `None`.
    ///
    /// # Example
    /// ```
    /// let host = request.header("host").expect("header not found");
    /// ```
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.to_lowercase() == name.to_lowercase())
            .map(|(_, value)| value.as_str())
    }
}

/// # HttpResponse
/// The IC data type for a response.
/// - `status_code`: an HTTP status code.
/// - `headers`: a list of key-value pairs.
/// - `streaming_strategy`: a streaming strategy for chunking the response.
/// - `body`: a raw byte array for the response body.
/// - `upgrade`: a flag to indicate whether the response should be upgraded to an update call.
///              This adds time (consensus) to the call, but allows for state changes to be commited.
///              Otherwise, the call is read-only and any changes done will be dropped
///
/// # Example
///
/// ```rs
/// // create a new ok response with a body
/// let res = HttpResponse::ok().body("Hello World");
///
/// // create a new custom response with headers
/// let res = HttpResponse::new(404)
///                     .headers(vec![("Content-Type", "text/html")])
///                     .body("<h1>Not Found</h1>");
/// ```
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HttpResponse {
    pub status_code: u16,
    pub headers: Vec<HeaderField>,
    pub body: Vec<u8>,
    pub streaming_strategy: Option<StreamingStrategy>,
    pub upgrade: bool,
}

impl HttpResponse {
    /// Create a new empty [`HttpResponse`] with a 200 status code. Generally used in conjunction with
    /// [`HttpResponse::body`] and [`HttpResponse::headers`]
    ///
    /// ```
    /// use ic_kit_http::HttpResponse;
    /// let res = HttpResponse::ok().body("Hello World");
    /// ```
    pub fn ok() -> Self {
        Self {
            status_code: 200,
            headers: vec![],
            body: vec![],
            streaming_strategy: None,
            upgrade: false,
        }
    }

    /// Create a new empty [`HttpResponse`] with a given status code.
    ///
    /// ```
    /// use ic_kit_http::HttpResponse;
    /// let res = HttpResponse::new(404);
    /// ```
    pub fn new(status_code: u16) -> Self {
        Self {
            status_code,
            headers: vec![],
            body: vec![],
            streaming_strategy: None,
            upgrade: false,
        }
    }

    /// Set the body of the [`HttpResponse`].
    /// ```
    /// use ic_kit_http::HttpResponse;
    /// let res = HttpResponse::ok();
    ///
    /// res.body("Hello World");
    /// ```
    pub fn body<T: Into<Vec<u8>>>(mut self, body: T) -> Self {
        self.body = body.into();
        self
    }

    /// Extend the existing body of the [`HttpResponse`].
    /// ```
    /// use ic_kit_http::HttpResponse;
    /// let res = HttpResponse::ok();
    ///
    /// res.body("Hello");
    /// res.extend_body(" World");
    /// ```
    ///
    pub fn extend_body<T: Into<Vec<u8>>>(mut self, body: T) -> Self {
        self.body.extend(body.into());
        self
    }

    /// Insert a header into the [`HttpResponse`]. If the header already exists, it will be replaced.
    /// If the value is empty, the header will be removed.
    pub fn header<T: Into<String>>(mut self, name: T, value: T) -> Self {
        let (name, value) = (name.into(), value.into());
        let mut map = self
            .headers
            .into_iter()
            .collect::<HashMap<String, String>>();
        if value.is_empty() {
            map.remove(&name);
        } else {
            map.insert(name, value);
        }
        self.headers = map.into_iter().collect();

        self
    }

    /// Set the headers of the [`HttpResponse`].
    ///
    /// To remove a header, set it to an empty string.
    ///
    /// ```
    /// use ic_kit_http::HttpResponse;
    /// let res = HttpResponse::ok();
    ///
    /// // set some headers
    /// res.headers(vec![
    ///     ("Content-Type".into(), "text/html".into()),
    ///     ("X-Foo".into(), "Bar".into()),
    /// ]);
    ///
    /// // remove a header
    /// res.headers(vec![
    ///    ("X-Foo".into(), "".into()),
    /// ]);
    ///
    /// ```
    pub fn headers(mut self, headers: Vec<HeaderField>) -> Self {
        let mut map = self
            .headers
            .into_iter()
            .collect::<HashMap<String, String>>();
        for (name, value) in headers {
            if value.is_empty() {
                map.remove(&name);
            } else {
                map.insert(name, value);
            }
        }
        self.headers = map.into_iter().collect();

        self
    }

    pub fn streaming_strategy(mut self, streaming_strategy: StreamingStrategy) -> Self {
        self.streaming_strategy = Some(streaming_strategy);
        self
    }

    #[inline(always)]
    pub fn upgrade(mut self) -> Self {
        self.upgrade = true;
        self
    }
}

/// # StreamingCallbackToken
/// The IC data type for a streaming callback token.
/// - `key`: the key of the resource to stream.
/// - `content_encoding`: the content encoding of the resource.
/// - `index`: the index to be used to identify the chunk or byte offset of the resource.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct StreamingCallbackToken {
    pub key: String,
    pub content_encoding: String,
    pub index: Nat,
}

/// # StreamingStrategy
/// The IC data type for a streaming strategy.
/// A streaming strategy is used to chunk the response body into multiple chunks.
///
/// ## Chunking Strategies
///
/// ### Callback
/// The `StreamingCallbackToken` is used to retrieve the next chunk of the response body.
/// The callback [`Func`] is a candid method that accepts a [`StreamingCallbackToken`]
/// and returns another [`StreamingCallbackHttpResponse`], for the next chunk of the response body.
/// These chunks are then streamed to the client by the gateway until there is no more callback tokens.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum StreamingStrategy {
    Callback {
        callback: Func,
        token: StreamingCallbackToken,
    },
}

/// # StreamingCallbackHttpResponse
/// The IC data type for a streaming callback response.
/// - `body`: a raw byte array for the chunk of the response body.
/// - `token`: a [`StreamingCallbackToken`] for the next chunk of the response body. If the token is `None`, then there are no more chunks.
#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct StreamingCallbackHttpResponse {
    pub body: Vec<u8>,
    pub token: Option<StreamingCallbackToken>,
}
