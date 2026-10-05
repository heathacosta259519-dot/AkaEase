use crate::error::{BackendError, Result};
use cookie_store::CookieStore;
use reqwest::{Url, header::HeaderValue};
use std::sync::Mutex;
use zeroize::Zeroizing;

/// Cannot be formatted or serialized into a frontend response.
pub(crate) struct StoredSession(pub(crate) Zeroizing<Vec<u8>>);

#[derive(Default)]
pub(crate) struct SessionCookies(Mutex<CookieStore>);

impl SessionCookies {
    pub(crate) fn csrf_token(&self, origin: &Url) -> Result<String> {
        let store = self.0.lock().map_err(|_| BackendError::CredentialStorage)?;
        Ok(store
            .get_request_values(origin)
            .find_map(|(name, value)| (name == "__csrf").then(|| value.to_owned()))
            .unwrap_or_default())
    }

    pub(crate) fn weapi_cookie_header(&self, origin: &Url) -> Result<HeaderValue> {
        let store = self.0.lock().map_err(|_| BackendError::CredentialStorage)?;
        let mut parts = vec!["os=pc".to_owned(), "appver=2.7.1.198277".to_owned()];
        parts.extend(
            store
                .get_request_values(origin)
                .filter(|(name, _)| *name != "os" && *name != "appver")
                .map(|(name, value)| format!("{name}={value}")),
        );
        HeaderValue::from_str(&parts.join("; "))
            .map_err(|_| BackendError::Protocol("invalid login cookie header".into()))
    }

    pub(crate) fn restore(secret: &StoredSession) -> Result<Self> {
        if secret.0.len() > 64 * 1024 {
            return Err(BackendError::CredentialStorage);
        }
        let store = cookie_store::serde::json::load(secret.0.as_slice())
            .map_err(|_| BackendError::CredentialStorage)?;
        Ok(Self(Mutex::new(store)))
    }
    pub(crate) fn export(&self) -> Result<StoredSession> {
        let store = self.0.lock().map_err(|_| BackendError::CredentialStorage)?;
        let mut bytes = Zeroizing::new(Vec::new());
        // Session cookies are necessary for restart, but absolute expiries remain intact.
        serde_json::to_writer(&mut *bytes, &store.iter_unexpired().collect::<Vec<_>>())
            .map_err(|_| BackendError::CredentialStorage)?;
        if bytes.len() > 64 * 1024 {
            return Err(BackendError::CredentialStorage);
        }
        Ok(StoredSession(bytes))
    }
    pub(crate) fn has_login_cookie(&self, origin: &Url) -> bool {
        self.0.lock().is_ok_and(|store| {
            store
                .get_request_values(origin)
                .any(|(name, value)| name == "MUSIC_U" && !value.is_empty())
        })
    }
}

impl reqwest::cookie::CookieStore for SessionCookies {
    fn set_cookies(&self, headers: &mut dyn Iterator<Item = &HeaderValue>, url: &Url) {
        if let Ok(mut store) = self.0.lock() {
            for header in headers {
                if let Ok(value) = header.to_str() {
                    let _ = store.parse(value, url);
                }
            }
        }
    }
    fn cookies(&self, url: &Url) -> Option<HeaderValue> {
        let store = self.0.lock().ok()?;
        let parts = store
            .get_request_values(url)
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>();
        if parts.is_empty() {
            None
        } else {
            HeaderValue::from_str(&parts.join("; ")).ok()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::cookie::CookieStore as _;
    #[test]
    fn preserves_scope_expiry_and_isolation() {
        let jar = SessionCookies::default();
        let origin = Url::parse("https://music.163.com/").unwrap();
        for value in [
            "MUSIC_U=synthetic; Path=/; Secure; HttpOnly",
            "expired=x; Max-Age=0; Path=/",
            "path=only; Path=/private; Secure",
        ] {
            jar.set_cookies(
                &mut [&HeaderValue::from_str(value).unwrap()].into_iter(),
                &origin,
            );
        }
        let restored = SessionCookies::restore(&jar.export().unwrap()).unwrap();
        assert!(!jar.export().unwrap().0.contains(&b'\n'));
        assert!(restored.has_login_cookie(&origin));
        assert_eq!(restored.cookies(&origin).unwrap(), "MUSIC_U=synthetic");
        assert!(
            restored
                .cookies(&Url::parse("http://music.163.com/").unwrap())
                .is_none()
        );
        assert!(
            restored
                .cookies(&Url::parse("https://other.invalid/").unwrap())
                .is_none()
        );
        assert!(!SessionCookies::default().has_login_cookie(&origin));
        assert_eq!(
            restored.weapi_cookie_header(&origin).unwrap(),
            "os=pc; appver=2.7.1.198277; MUSIC_U=synthetic"
        );
    }
}
