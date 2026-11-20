mod support;

use backend::unleashed::session::{Session, SessionError};
use support::{Mock, PASSWORD, USERNAME};

const LIST: &str = "<ajax-request action='getconf' DECRYPT_X='true' updater='guest-list.1' comp='guest-list'><guest self-service='!true'/></ajax-request>";

fn session(mock: &Mock, password: &str) -> Session {
    Session::new(&mock.url, USERNAME, password, true).unwrap()
}

#[tokio::test]
async fn logs_in_and_lists() {
    let mock = Mock::start().await;
    let s = session(&mock, PASSWORD);
    s.login().await.unwrap();
    let xml = s.conf(LIST).await.unwrap();
    assert!(xml.contains("<resultset>"), "{xml}");
    assert_eq!(mock.logins(), 1);
}

#[tokio::test]
async fn logs_in_lazily_on_first_request() {
    let mock = Mock::start().await;
    let s = session(&mock, PASSWORD);
    assert!(s.conf(LIST).await.unwrap().contains("<resultset>"));
    assert_eq!(mock.logins(), 1);
}

#[tokio::test]
async fn rejects_bad_credentials() {
    let mock = Mock::start().await;
    let s = session(&mock, "wrong");
    assert!(matches!(s.login().await, Err(SessionError::Auth(_))));
}

#[tokio::test]
async fn reads_the_token_from_the_script_on_older_firmware() {
    let mock = Mock::start().await;
    mock.state.lock().unwrap().token_in_script_only = true;
    let s = session(&mock, PASSWORD);
    assert!(s.conf(LIST).await.unwrap().contains("<resultset>"));
}

#[tokio::test]
async fn logs_in_again_when_the_session_expires() {
    let mock = Mock::start().await;
    let s = session(&mock, PASSWORD);
    s.conf(LIST).await.unwrap();
    mock.expire_sessions();
    assert!(s.conf(LIST).await.unwrap().contains("<resultset>"));
    assert_eq!(mock.logins(), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_requests_share_one_relogin() {
    let mock = Mock::start().await;
    let s = std::sync::Arc::new(session(&mock, PASSWORD));
    s.conf(LIST).await.unwrap();
    mock.expire_sessions();
    let calls = (0..5).map(|_| {
        let s = s.clone();
        tokio::spawn(async move { s.conf(LIST).await })
    });
    for call in calls {
        assert!(call.await.unwrap().is_ok());
    }
    assert_eq!(mock.logins(), 2);
}

#[tokio::test]
async fn gives_up_after_one_relogin() {
    let mock = Mock::start().await;
    let s = session(&mock, PASSWORD);
    s.login().await.unwrap();
    mock.state.lock().unwrap().always_redirect = true;
    assert!(matches!(s.conf(LIST).await, Err(SessionError::Auth(_))));
    assert_eq!(mock.logins(), 2);
}

#[tokio::test]
async fn unreachable_controller_is_a_connect_error() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let s = Session::new(&url, USERNAME, PASSWORD, true).unwrap();
    let err = s.login().await.unwrap_err();
    assert!(matches!(err, SessionError::Connect(_)));
    // The cause, not just reqwest's "error sending request", reaches the log.
    assert!(err.to_string().to_lowercase().contains("refused"), "{err}");
}

#[tokio::test]
async fn login_errors_never_contain_the_password() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let unreachable = Session::new(&url, USERNAME, PASSWORD, true).unwrap();
    let err = unreachable.login().await.unwrap_err().to_string();
    assert!(
        !err.contains(PASSWORD) && !err.contains("password="),
        "{err}"
    );

    let mock = Mock::start().await;
    mock.state.lock().unwrap().login_without_location = true;
    let err = session(&mock, PASSWORD)
        .login()
        .await
        .unwrap_err()
        .to_string();
    assert!(
        !err.contains(PASSWORD) && !err.contains("password="),
        "{err}"
    );
}

#[tokio::test]
async fn posts_forms() {
    let mock = Mock::start().await;
    let s = session(&mock, PASSWORD);
    let body = s
        .form(
            "mon_createguest.jsp",
            &[
                ("gentype", "single".to_string()),
                ("fullname", "lobby".to_string()),
            ],
        )
        .await
        .unwrap();
    assert!(body.contains("\"result\":\"DONE\""), "{body}");
    assert_eq!(
        mock.state.lock().unwrap().create_forms[0]["fullname"],
        "lobby"
    );
}
