use candid::{CandidType, Deserialize, Func, Nat};
pub use matchit::{Match, MatchError, Params, Router as BasicRouter};

pub use ic_kit_macros::{delete, get, post, put};

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
