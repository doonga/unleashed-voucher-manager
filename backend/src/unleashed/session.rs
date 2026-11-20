//! An authenticated session with the controller's admin pages.

use std::{fmt, time::Duration};

use reqwest::{Client, StatusCode, Url, header::LOCATION, redirect::Policy};
use tokio::sync::Mutex;
use tracing::{debug, info};

const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_LOGIN_HOPS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// The controller could not be reached or answered unexpectedly.
    Connect(String),
    /// The controller refused the credentials or the session.
    Auth(String),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Connect(e) => write!(f, "controller request failed: {e}"),
            Self::Auth(e) => write!(f, "controller authentication failed: {e}"),
        }
    }
}

impl std::error::Error for SessionError {}

#[derive(Clone)]
struct Login {
    /// `https://host/admin/`, the directory of the login page.
    admin: Url,
    token: String,
}

struct State {
    login: Option<Login>,
    /// Bumped on every login, so concurrent callers that saw the same
    /// expired session log in once between them.
    generation: u64,
}

pub struct Session {
    client: Client,
    url: Url,
    username: String,
    password: String,
    state: Mutex<State>,
}

/// A reqwest error without the request URL, which for a login carries the
/// password in its query string.
fn connect_error(e: reqwest::Error) -> SessionError {
    // reqwest's own message is only the kind of failure ("error sending
    // request"); the cause, such as a refused connection or an untrusted
    // certificate, is in the source chain, which carries no URL.
    let e = e.without_url();
    let mut message = e.to_string();
    let mut source = std::error::Error::source(&e);
    while let Some(cause) = source {
        let text = cause.to_string();
        if !message.contains(&text) {
            message = format!("{message}: {text}");
        }
        source = cause.source();
    }
    SessionError::Connect(message)
}

/// A URL with any query string removed, safe to put in errors and logs.
fn redacted(url: &Url) -> String {
    let mut url = url.clone();
    url.set_query(None);
    url.to_string()
}

enum Body<'a> {
    Xml(&'a str),
    Form(&'a [(&'static str, String)]),
}

impl Session {
    pub fn new(
        url: &str,
        username: &str,
        password: &str,
        verify_tls: bool,
    ) -> Result<Self, SessionError> {
        let url = Url::parse(url).map_err(|e| SessionError::Connect(format!("bad URL: {e}")))?;
        let client = Client::builder()
            .cookie_store(true)
            .redirect(Policy::none())
            .timeout(REQUEST_TIMEOUT)
            .danger_accept_invalid_certs(!verify_tls)
            .build()
            .map_err(connect_error)?;
        Ok(Self {
            client,
            url,
            username: username.to_string(),
            password: password.to_string(),
            state: Mutex::new(State {
                login: None,
                generation: 0,
            }),
        })
    }

    /// Log in now rather than on the first request.
    pub async fn login(&self) -> Result<(), SessionError> {
        let mut state = self.state.lock().await;
        state.login = Some(self.log_in().await?);
        state.generation += 1;
        Ok(())
    }

    /// POST an AJAX request to `_conf.jsp` and return the XML answer.
    pub async fn conf(&self, xml: &str) -> Result<String, SessionError> {
        self.send("_conf.jsp", Body::Xml(xml)).await
    }

    /// POST a form to an admin page and return the answer.
    pub async fn form(
        &self,
        page: &str,
        fields: &[(&'static str, String)],
    ) -> Result<String, SessionError> {
        self.send(page, Body::Form(fields)).await
    }

    async fn current(&self) -> Result<(Login, u64), SessionError> {
        let mut state = self.state.lock().await;
        if state.login.is_none() {
            state.login = Some(self.log_in().await?);
            state.generation += 1;
        }
        Ok((state.login.clone().expect("logged in"), state.generation))
    }

    /// Log in again unless another caller already did since `seen`.
    async fn relogin(&self, seen: u64) -> Result<(), SessionError> {
        let mut state = self.state.lock().await;
        if state.generation == seen {
            info!("Controller session expired; logging in again");
            state.login = None;
            state.login = Some(self.log_in().await?);
            state.generation += 1;
        }
        Ok(())
    }

    async fn send(&self, page: &str, body: Body<'_>) -> Result<String, SessionError> {
        let (login, generation) = self.current().await?;
        if let Some(text) = self.post(&login, page, &body).await? {
            return Ok(text);
        }
        self.relogin(generation).await?;
        let (login, _) = self.current().await?;
        self.post(&login, page, &body).await?.ok_or_else(|| {
            SessionError::Auth("the controller refused the session after logging in again".into())
        })
    }

    /// `None` when the controller redirects to its login page.
    async fn post(
        &self,
        login: &Login,
        page: &str,
        body: &Body<'_>,
    ) -> Result<Option<String>, SessionError> {
        let url = login
            .admin
            .join(page)
            .map_err(|e| SessionError::Connect(e.to_string()))?;
        let request = self.client.post(url).header("X-CSRF-Token", &login.token);
        let request = match body {
            Body::Xml(xml) => request
                .header("Content-Type", "text/xml")
                .body(xml.to_string()),
            Body::Form(fields) => request.form(fields),
        };
        let response = request.send().await.map_err(connect_error)?;
        match response.status() {
            StatusCode::FOUND => Ok(None),
            status if status.is_success() => {
                Ok(Some(response.text().await.map_err(connect_error)?))
            }
            status => Err(SessionError::Connect(format!("{page} answered {status}"))),
        }
    }

    fn location(response: &reqwest::Response) -> Result<Url, SessionError> {
        let location = response
            .headers()
            .get(LOCATION)
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| {
                SessionError::Connect(format!(
                    "expected a redirect from {}, got {}",
                    redacted(response.url()),
                    response.status()
                ))
            })?;
        response
            .url()
            .join(location)
            .map_err(|e| SessionError::Connect(e.to_string()))
    }

    async fn log_in(&self) -> Result<Login, SessionError> {
        // Follow the redirects from `/` to the login page.
        let mut login_url = self.url.clone();
        for _ in 0..MAX_LOGIN_HOPS {
            if login_url.path().ends_with("login.jsp") {
                break;
            }
            let response = self
                .client
                .get(login_url.clone())
                .send()
                .await
                .map_err(connect_error)?;
            login_url = Self::location(&response)?;
        }
        if !login_url.path().ends_with("login.jsp") {
            return Err(SessionError::Connect(
                "could not find the login page".into(),
            ));
        }
        debug!("Logging in at {}", redacted(&login_url));
        let admin = login_url
            .join("./")
            .map_err(|e| SessionError::Connect(e.to_string()))?;

        let response = self
            .client
            .get(login_url)
            .query(&[
                ("username", self.username.as_str()),
                ("password", self.password.as_str()),
                ("ok", "Log In"),
            ])
            .send()
            .await
            .map_err(connect_error)?;
        if response.status() != StatusCode::FOUND {
            return Err(SessionError::Auth(
                "the controller refused the username or password".into(),
            ));
        }
        let dashboard = Self::location(&response)?;
        let header_token = response
            .headers()
            .get("HTTP_X_CSRF_TOKEN")
            .and_then(|v| v.to_str().ok())
            .filter(|t| !t.is_empty())
            .map(str::to_string);

        // The session only accepts AJAX calls after the dashboard has loaded.
        self.client
            .get(dashboard)
            .send()
            .await
            .map_err(connect_error)?;

        let token = match header_token {
            Some(token) => token,
            None => self.script_token(&admin).await?,
        };
        Ok(Login { admin, token })
    }

    /// Older firmware only exposes the CSRF token from a script.
    async fn script_token(&self, admin: &Url) -> Result<String, SessionError> {
        let url = admin
            .join("_csrfTokenVar.jsp")
            .map_err(|e| SessionError::Connect(e.to_string()))?;
        let text = self
            .client
            .get(url)
            .send()
            .await
            .map_err(connect_error)?
            .text()
            .await
            .map_err(connect_error)?;
        text.split('\'')
            .nth(1)
            .filter(|t| !t.is_empty())
            .map(str::to_string)
            .ok_or_else(|| SessionError::Auth("the controller sent no CSRF token".into()))
    }
}
