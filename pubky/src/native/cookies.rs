use std::{collections::HashMap, sync::RwLock};

use pkarr::PublicKey;
use reqwest::{cookie::CookieStore, header::HeaderValue, Response};

#[derive(Default)]
pub struct CookieJar {
    pubky_sessions: RwLock<HashMap<String, String>>,
    normal_jar: RwLock<cookie_store::CookieStore>,
}

impl CookieJar {
    pub(crate) fn store_session_after_signup(&self, response: &Response, pubky: &PublicKey) {
        for (header_name, header_value) in response.headers() {
            if header_name == "set-cookie" && header_value.as_ref().starts_with(b"session_id=") {
                if let Ok(Ok(cookie)) =
                    std::str::from_utf8(header_value.as_bytes()).map(cookie::Cookie::parse)
                {
                    if cookie.name() == "session_id" {
                        let domain = format!("_pubky.{pubky}");
                        tracing::debug!(?cookie, "Storing coookie after signup");

                        self.pubky_sessions
                            .write()
                            .unwrap()
                            .insert(domain, cookie.value().to_string());
                    }
                };
            }
        }
    }

    pub(crate) fn delete_session_after_signout(&self, pubky: &PublicKey) {
        self.pubky_sessions
            .write()
            .unwrap()
            .remove(&format!("_pubky.{pubky}"));
    }
}

impl CookieStore for CookieJar {
    fn set_cookies(&self, cookie_headers: &mut dyn Iterator<Item = &HeaderValue>, url: &url::Url) {
        let iter = cookie_headers.filter_map(|val| {
            val.to_str()
                .ok()
                .and_then(|s| cookie::Cookie::parse(s.to_owned()).ok())
        });

        self.normal_jar
            .write()
            .unwrap()
            .store_response_cookies(iter, url);
    }

    fn cookies(&self, url: &url::Url) -> Option<HeaderValue> {
        let s = self
            .normal_jar
            .read()
            .unwrap()
            .get_request_values(url)
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("; ");

        if s.is_empty() {
            return self
                .pubky_sessions
                .read()
                .unwrap()
                .get(url.host_str().unwrap())
                .map(|secret| HeaderValue::try_from(format!("session_id={secret}")).unwrap());
        }

        HeaderValue::from_maybe_shared(bytes::Bytes::from(s)).ok()
    }
}
