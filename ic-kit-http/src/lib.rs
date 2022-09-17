use candid::{CandidType, Deserialize, Func, Nat};

pub use ic_kit_macros::{delete, get, post, put};
pub use matchit::{Match, MatchError, Params, Router as BasicRouter};

pub type HeaderField = (String, String);

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HttpRequest {
    pub method: String,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl HttpRequest {
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .iter()
            .find(|(key, _)| key.to_lowercase() == name.to_lowercase())
            .map(|(_, value)| value.as_str())
    }
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct HttpResponse {
    pub status_code: u16,
    pub headers: Vec<HeaderField>,
    pub body: Vec<u8>,
    pub streaming_strategy: Option<StreamingStrategy>,
    pub upgrade: bool,
}

impl HttpResponse {
    pub fn ok() -> Self {
        Self {
            status_code: 200,
            headers: vec![],
            body: vec![],
            streaming_strategy: None,
            upgrade: false,
        }
    }

    pub fn new(status_code: u16) -> Self {
        Self {
            status_code,
            headers: vec![],
            body: vec![],
            streaming_strategy: None,
            upgrade: false,
        }
    }

    #[inline(always)]
    pub fn with_body<T: Into<Vec<u8>>>(mut self, body: T) -> Self {
        self.body = body.into();
        self
    }

    #[inline(always)]
    pub fn with_header(mut self, name: &str, value: &str) -> Self {
        self.headers.push((name.to_string(), value.to_string()));
        self
    }

    #[inline(always)]
    pub fn with_headers(mut self, headers: Vec<HeaderField>) -> Self {
        self.headers.extend(headers);
        self
    }

    #[inline(always)]
    pub fn with_streaming_strategy(mut self, streaming_strategy: StreamingStrategy) -> Self {
        self.streaming_strategy = Some(streaming_strategy);
        self
    }

    #[inline(always)]
    pub fn with_upgrade(mut self) -> Self {
        self.upgrade = true;
        self
    }
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct StreamingCallbackToken {
    pub key: String,
    pub content_encoding: String,
    pub index: Nat,
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub enum StreamingStrategy {
    Callback {
        callback: Func,
        token: StreamingCallbackToken,
    },
}

#[derive(Clone, Debug, CandidType, Deserialize)]
pub struct StreamingCallbackHttpResponse {
    pub body: Vec<u8>,
    pub token: Option<StreamingCallbackToken>,
}

/// A [`Method`](https://developer.mozilla.org/en-US/docs/Web/API/Request/method) representation
/// used on Request objects.
#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub enum Method {
    Head = 0,
    Get,
    Post,
    Put,
    Patch,
    Delete,
    Options,
    Connect,
    Trace,
}

impl Method {
    pub fn all() -> Vec<Method> {
        vec![
            Method::Head,
            Method::Get,
            Method::Post,
            Method::Put,
            Method::Patch,
            Method::Delete,
            Method::Options,
            Method::Connect,
            Method::Trace,
        ]
    }
}

impl From<String> for Method {
    fn from(m: String) -> Self {
        match m.to_ascii_uppercase().as_str() {
            "HEAD" => Method::Head,
            "POST" => Method::Post,
            "PUT" => Method::Put,
            "PATCH" => Method::Patch,
            "DELETE" => Method::Delete,
            "OPTIONS" => Method::Options,
            "CONNECT" => Method::Connect,
            "TRACE" => Method::Trace,
            _ => Method::Get,
        }
    }
}

impl From<Method> for String {
    fn from(val: Method) -> Self {
        val.as_ref().to_string()
    }
}

impl AsRef<str> for Method {
    fn as_ref(&self) -> &'static str {
        match self {
            Method::Head => "HEAD",
            Method::Post => "POST",
            Method::Put => "PUT",
            Method::Patch => "PATCH",
            Method::Delete => "DELETE",
            Method::Options => "OPTIONS",
            Method::Connect => "CONNECT",
            Method::Trace => "TRACE",
            Method::Get => "GET",
        }
    }
}

impl ToString for Method {
    fn to_string(&self) -> String {
        (*self).clone().into()
    }
}

impl Default for Method {
    fn default() -> Self {
        Method::Get
    }
}
