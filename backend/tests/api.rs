mod support;

use std::sync::Arc;

use axum::http::StatusCode;
use backend::{
    models::{VouchersCreateRequest, VouchersGetRequest},
    unleashed::session::SessionError,
    unleashed_api::{ApiConfig, ApiError, UnleashedAPI},
};
use support::{Mock, PASSWORD, USERNAME};

fn config(mock: &Mock, password: &str) -> ApiConfig {
    ApiConfig {
        url: mock.url.clone(),
        username: USERNAME.to_string(),
        password: password.to_string(),
        ssid: "Guest".to_string(),
        verify_tls: true,
        timezone: chrono_tz::America::Toronto,
        rolling_voucher_duration_minutes: 480,
    }
}

async fn api(mock: &Mock) -> UnleashedAPI {
    UnleashedAPI::try_new(config(mock, PASSWORD)).await.unwrap()
}

fn request(count: u32, name: &str) -> VouchersCreateRequest {
    VouchersCreateRequest {
        count,
        name: name.to_string(),
        authorized_guest_limit: None,
        time_limit_minutes: 1440,
        code: None,
        remarks: None,
    }
}

fn status<T>(result: Result<T, ApiError>) -> StatusCode {
    match result {
        Ok(_) => StatusCode::OK,
        Err(e) => e.status,
    }
}

#[tokio::test]
async fn startup_fails_on_bad_credentials() {
    let mock = Mock::start().await;
    let result = UnleashedAPI::try_new(config(&mock, "wrong")).await;
    assert!(matches!(result, Err(SessionError::Auth(_))));
}

#[tokio::test]
async fn creates_a_single_pass_with_a_spaced_name() {
    let mock = Mock::start().await;
    let api = api(&mock).await;
    let created = api
        .create_voucher(&request(1, "Quick Voucher"))
        .await
        .unwrap();
    assert_eq!(created.vouchers.len(), 1);
    assert_eq!(created.vouchers[0].name, "Quick-Voucher");
    assert_eq!(created.vouchers[0].time_limit_minutes, 1440);
}

#[tokio::test]
async fn duplicate_names_are_fine() {
    let mock = Mock::start().await;
    let api = api(&mock).await;
    api.create_voucher(&request(1, "lobby")).await.unwrap();
    api.create_voucher(&request(1, "lobby")).await.unwrap();
    assert_eq!(mock.pass_ids().len(), 2);
}

#[tokio::test]
async fn creates_a_batch() {
    let mock = Mock::start().await;
    let api = api(&mock).await;
    let created = api.create_voucher(&request(3, "ignored")).await.unwrap();
    assert_eq!(created.vouchers.len(), 3);
    assert!(
        created
            .vouchers
            .iter()
            .all(|v| v.name.starts_with("Guest-"))
    );
}

#[tokio::test]
async fn custom_key_and_remarks() {
    let mock = Mock::start().await;
    let api = api(&mock).await;
    let mut r = request(1, "front");
    r.code = Some("check1".to_string());
    r.remarks = Some("Bob's phone".to_string());
    r.authorized_guest_limit = Some(3);
    let created = api.create_voucher(&r).await.unwrap();
    let v = &created.vouchers[0];
    assert_eq!(v.code, "CHECK1");
    assert_eq!(v.remarks, "Bob's phone");
    assert_eq!(v.authorized_guest_limit, Some(3));
}

#[tokio::test]
async fn duplicate_key_is_a_conflict() {
    let mock = Mock::start().await;
    let api = api(&mock).await;
    let mut r = request(1, "a");
    r.code = Some("SAME1".to_string());
    api.create_voucher(&r).await.unwrap();
    let err = api.create_voucher(&r).await.unwrap_err();
    assert_eq!(err.status, StatusCode::CONFLICT);
    assert!(err.message.contains("already exists"), "{}", err.message);
}

#[tokio::test]
async fn invalid_requests_are_bad_requests() {
    let mock = Mock::start().await;
    let api = api(&mock).await;
    let mut keyed_batch = request(2, "a");
    keyed_batch.code = Some("ABC".to_string());
    let mut bad_remarks = request(1, "a");
    bad_remarks.remarks = Some("<b>".to_string());
    for r in [
        request(0, "a"),
        request(101, "a"),
        keyed_batch,
        request(1, "a&b"),
        bad_remarks,
    ] {
        let err = api.create_voucher(&r).await.unwrap_err();
        assert_eq!(err.status, StatusCode::BAD_REQUEST, "{r:?}");
        assert!(!err.message.is_empty());
    }
    assert!(mock.state.lock().unwrap().create_forms.is_empty());
}

#[tokio::test]
async fn controller_character_rejection_is_a_bad_request() {
    let mock = Mock::start().await;
    let api = api(&mock).await;
    mock.state.lock().unwrap().reject_as_invalid = true;
    let err = api.create_voucher(&request(1, "a")).await.unwrap_err();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert!(
        err.message.contains("Invalid Characters"),
        "{}",
        err.message
    );
}

#[tokio::test]
async fn an_ok_that_creates_nothing_is_an_error() {
    let mock = Mock::start().await;
    let api = api(&mock).await;
    mock.state.lock().unwrap().ignore_creates = true;
    assert_eq!(
        status(api.create_voucher(&request(1, "a")).await),
        StatusCode::BAD_GATEWAY
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_creates_return_only_their_own_pass() {
    let mock = Mock::start().await;
    mock.state.lock().unwrap().create_delay_ms = 50;
    let api = Arc::new(api(&mock).await);
    let (a, b) = tokio::join!(
        {
            let api = api.clone();
            async move { api.create_voucher(&request(1, "first")).await }
        },
        {
            let api = api.clone();
            async move { api.create_voucher(&request(1, "second")).await }
        }
    );
    let a = a.unwrap().vouchers;
    let b = b.unwrap().vouchers;
    assert_eq!(a.len(), 1);
    assert_eq!(b.len(), 1);
    assert_eq!(a[0].name, "first");
    assert_eq!(b[0].name, "second");
}

#[tokio::test]
async fn lists_newest_first_and_filters_by_name() {
    let mock = Mock::start().await;
    let api = api(&mock).await;
    for name in ["alpha", "beta", "alphabet"] {
        api.create_voucher(&request(1, name)).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    }
    let all = api.get_all_vouchers().await.unwrap();
    assert_eq!(all.total_count, 3);
    assert_eq!(all.data[0].name, "alphabet");

    let filtered = api
        .get_vouchers(&VouchersGetRequest {
            offset: 0,
            limit: 10,
            filter: Some("ALPHA".to_string()),
        })
        .await
        .unwrap();
    let names: Vec<_> = filtered.data.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["alphabet", "alpha"]);
    assert_eq!(filtered.total_count, 2);

    let page = api
        .get_vouchers(&VouchersGetRequest {
            offset: 1,
            limit: 1,
            filter: None,
        })
        .await
        .unwrap();
    assert_eq!(page.data.len(), 1);
    assert_eq!(page.data[0].name, "beta");
}

#[tokio::test]
async fn details_and_newest() {
    let mock = Mock::start().await;
    let api = api(&mock).await;
    let a = mock.add_pass("older", false, 3600);
    let b = mock.add_pass("newer", false, 3600);
    {
        let mut s = mock.state.lock().unwrap();
        s.passes.iter_mut().find(|p| p.id == b).unwrap().create_time += 10;
    }
    assert_eq!(
        api.get_voucher_details(a.to_string()).await.unwrap().name,
        "older"
    );
    assert_eq!(api.get_newest_voucher().await.unwrap().name, "newer");
    assert_eq!(
        status(api.get_voucher_details("999".to_string()).await),
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn deletes_selected_passes() {
    let mock = Mock::start().await;
    let api = api(&mock).await;
    let a = mock.add_pass("a", false, 3600);
    let b = mock.add_pass("b", false, 3600);
    let c = mock.add_pass("c", false, 3600);
    let deleted = api
        .delete_vouchers_by_ids(vec![a.to_string(), c.to_string()])
        .await
        .unwrap();
    assert_eq!(deleted.vouchers_deleted, 2);
    assert_eq!(mock.pass_ids(), vec![b]);
}

#[tokio::test]
async fn rejects_non_numeric_ids() {
    let mock = Mock::start().await;
    let api = api(&mock).await;
    let a = mock.add_pass("a", false, 3600);
    mock.add_pass("b", false, 3600);
    let err = api
        .delete_vouchers_by_ids(vec![format!("{a}'></guest><guest id='2")])
        .await
        .unwrap_err();
    assert_eq!(err.status, StatusCode::BAD_REQUEST);
    assert_eq!(mock.pass_ids().len(), 2);
}

#[tokio::test]
async fn deleting_nothing_is_fine() {
    let mock = Mock::start().await;
    let api = api(&mock).await;
    let deleted = api
        .delete_vouchers_by_ids(vec!["42".to_string()])
        .await
        .unwrap();
    assert_eq!(deleted.vouchers_deleted, 0);
}

#[tokio::test]
async fn rolling_vouchers_are_for_one_device() {
    let mock = Mock::start().await;
    let api = api(&mock).await;
    let v = api.create_rolling_voucher().await.unwrap();
    assert!(v.name.starts_with("[ROLLING]-"), "{}", v.name);
    assert_eq!(v.authorized_guest_limit, Some(1));
    assert_eq!(v.time_limit_minutes, 480);
    let forms = &mock.state.lock().unwrap().create_forms;
    assert_eq!(forms[0]["limitnumber"], "1");
}

#[tokio::test]
async fn one_rolling_voucher_waits_at_a_time() {
    let mock = Mock::start().await;
    let api = api(&mock).await;
    let first = api.create_rolling_voucher().await.unwrap();
    let again = api.create_rolling_voucher().await.unwrap();
    assert_eq!(first.id, again.id);
    assert_eq!(
        api.get_rolling_voucher().await.unwrap().unwrap().id,
        first.id
    );

    mock.mark_used(first.id.parse().unwrap());
    assert!(api.get_rolling_voucher().await.unwrap().is_none());
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    let next = api.create_rolling_voucher().await.unwrap();
    assert_ne!(next.id, first.id);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_rolling_requests_create_one_voucher() {
    let mock = Mock::start().await;
    mock.state.lock().unwrap().create_delay_ms = 50;
    let api = Arc::new(api(&mock).await);
    let calls: Vec<_> = (0..5)
        .map(|_| {
            let api = api.clone();
            tokio::spawn(async move { api.create_rolling_voucher().await })
        })
        .collect();
    let mut ids = Vec::new();
    for call in calls {
        ids.push(call.await.unwrap().unwrap().id);
    }
    ids.dedup();
    assert_eq!(ids.len(), 1, "{ids:?}");
    assert_eq!(mock.pass_ids().len(), 1);
}

#[tokio::test]
async fn expired_cleanup_skips_passes_without_expiry() {
    let mock = Mock::start().await;
    let api = api(&mock).await;
    let keep = mock.add_pass_without_expiry("no-expiry");
    let live = mock.add_pass("live", false, 3600);
    let deleted = api.delete_expired_vouchers().await.unwrap();
    assert_eq!(deleted.vouchers_deleted, 0);
    assert_eq!(mock.pass_ids(), vec![keep, live]);
}

#[tokio::test]
async fn survives_a_controller_reboot() {
    let mock = Mock::start().await;
    let api = api(&mock).await;
    api.get_all_vouchers().await.unwrap();
    mock.expire_sessions();
    api.create_voucher(&request(1, "after")).await.unwrap();
    assert_eq!(mock.logins(), 2);
}

#[tokio::test]
async fn a_delete_the_controller_ignored_is_an_error() {
    let mock = Mock::start().await;
    let api = api(&mock).await;
    let a = mock.add_pass("a", false, 3600);
    mock.state.lock().unwrap().ignore_deletes = true;
    let err = api
        .delete_vouchers_by_ids(vec![a.to_string()])
        .await
        .unwrap_err();
    assert_eq!(err.status, StatusCode::BAD_GATEWAY);
    assert!(err.message.contains("did not delete"), "{}", err.message);
}

#[tokio::test]
async fn expired_passes_the_controller_already_removed_are_not_a_failed_delete() {
    let mock = Mock::start().await;
    let api = api(&mock).await;
    let gone = mock.add_pass("gone", false, 3600);
    // Expire it without the mock's own list cleanup removing it.
    mock.state
        .lock()
        .unwrap()
        .passes
        .iter_mut()
        .find(|p| p.id == gone)
        .unwrap()
        .expire_time = Some(1);
    mock.state.lock().unwrap().ignore_deletes = true;
    let _ = api.delete_expired_vouchers().await;
    // The mock drops expired passes from its list, as the controller does, so
    // the pass is not seen again; the check must not report a failure for it.
    assert!(api.delete_expired_vouchers().await.is_ok());
}
