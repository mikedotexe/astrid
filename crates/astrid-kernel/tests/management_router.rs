//! Exercise the real router with a separate process and temporary Astrid home.

use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use astrid_core::SessionId;
use astrid_events::ipc::{IpcMessage, IpcPayload};
use astrid_events::kernel_api::{KernelRequest, KernelResponse};
use astrid_events::{AstridEvent, EventMetadata};

#[test]
fn rate_limited_responses_reach_management_subscribers() {
    const CHILD: &str = "ASTRID_ROUTER_TEST_CHILD";
    if std::env::var_os(CHILD).is_some() {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .unwrap()
            .block_on(async {
                tokio::time::timeout(Duration::from_secs(10), exercise_router())
                    .await
                    .expect("isolated kernel/router deadline");
            });
        return;
    }

    // Process-local environment avoids mutating HOME while other tests run.
    let home = tempfile::Builder::new()
        .prefix("astrid-router-test-")
        .tempdir()
        .unwrap();
    let mut child = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "rate_limited_responses_reach_management_subscribers",
            "--nocapture",
        ])
        .env(CHILD, "1")
        .env("ASTRID_HOME", home.path())
        .env("HOME", home.path())
        .current_dir(home.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let started = Instant::now();
    while child.try_wait().unwrap().is_none() {
        if started.elapsed() > Duration::from_secs(30) {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("isolated router test child exceeded deadline");
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "isolated router test failed:\n{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

async fn exercise_router() {
    let home = std::path::PathBuf::from(std::env::var_os("ASTRID_HOME").unwrap());
    assert!(
        home.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("astrid-router-test-")
    );
    let kernel = astrid_kernel::Kernel::new(SessionId::new(), home)
        .await
        .unwrap();
    let mut responses = kernel.event_bus.subscribe_topic("astrid.v1.response.*");
    // Include the old prefix inside the suffix: only the leading namespace maps.
    let topic = "astrid.v1.request.kernel.request.approval";
    for index in 0..11 {
        let request = KernelRequest::ApproveCapability {
            request_id: format!("synthetic-{index}"),
            signature: "synthetic-not-an-approval".to_string(),
        };
        kernel.event_bus.publish(AstridEvent::Ipc {
            metadata: EventMetadata::new("router_test"),
            message: IpcMessage::new(
                topic,
                IpcPayload::RawJson(serde_json::to_value(request).unwrap()),
                kernel.session_id.0,
            ),
        });
        let event = tokio::time::timeout(Duration::from_secs(2), responses.recv())
            .await
            .expect("every request, including rate rejection, must reach the response subscriber")
            .unwrap();
        let AstridEvent::Ipc { message, .. } = &*event else {
            panic!("expected IPC response");
        };
        assert_eq!(message.topic, "astrid.v1.response.kernel.request.approval");
        let IpcPayload::RawJson(payload) = &message.payload else {
            panic!("expected JSON response");
        };
        let KernelResponse::Error(error) = serde_json::from_value(payload.clone()).unwrap() else {
            panic!("approval is a stub, followed by rate rejection, never a grant");
        };
        if index < 10 {
            assert_eq!(error, "Approval logic not yet implemented in kernel router");
        } else {
            assert_eq!(
                error,
                "Rate limited: max 10 ApproveCapability requests per minute"
            );
        }
    }
}
