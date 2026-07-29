//! Live control-plane contract test against an ephemeral Temporal dev server.
//!
//! Install the pinned CLI with `scripts/install-temporal-cli.sh`, then run:
//! `cargo test --test live_temporal -- --ignored --nocapture`.

use std::{
    collections::HashMap,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use base64::prelude::*;
use temporal_tui::{
    model::{
        BatchOperationKind, BatchOperationRequest, Capability, CapabilityAvailability,
        ScheduleBackfillRequest, ScheduleCreateRequest, ScheduleUpdateRequest, WorkflowKey,
        WorkflowStatus,
    },
    service::{GrpcTemporalService, PayloadCodecConfig, TemporalConnectionConfig, TemporalService},
};
use temporalio_client::{Client, ClientOptions, Connection, ConnectionOptions};
use temporalio_common::{
    protos::temporal::api::enums::v1::VersioningBehavior,
    worker::{WorkerDeploymentOptions, WorkerDeploymentVersion, WorkerTaskTypes},
};
use temporalio_macros::{workflow, workflow_methods};
use temporalio_sdk::{
    SyncWorkflowContext, Worker, WorkerOptions, WorkflowContext, WorkflowContextView,
    WorkflowResult,
};
use temporalio_sdk_core::{CoreRuntime, RuntimeOptions};
use tokio::time::sleep;

#[workflow]
#[derive(Default)]
struct TemporalTuiControlWorkflow {
    counter: i32,
}

#[workflow_methods]
impl TemporalTuiControlWorkflow {
    #[run]
    async fn run(ctx: &mut WorkflowContext<Self>, initial: i32) -> WorkflowResult<i32> {
        ctx.state_mut(|state| state.counter = initial);
        ctx.wait_condition(|state| state.counter >= 1_000).await;
        Ok(ctx.state(|state| state.counter))
    }

    #[query(name = "current")]
    fn current(&self, _ctx: &WorkflowContextView) -> i32 {
        self.counter
    }

    #[update(name = "set")]
    fn set(&mut self, _ctx: &mut SyncWorkflowContext<Self>, value: i32) -> i32 {
        let previous = self.counter;
        self.counter = value;
        previous
    }
}

struct DevServer {
    child: Child,
}

impl Drop for DevServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct CodecServer {
    address: String,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}

impl CodecServer {
    fn start() -> Self {
        let listener = TcpListener::bind(("127.0.0.1", 0)).expect("bind Codec Server");
        let address = listener.local_addr().expect("Codec Server address");
        listener
            .set_nonblocking(true)
            .expect("non-blocking Codec Server");
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = Arc::clone(&stop);
        let thread = thread::spawn(move || {
            while !thread_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((mut stream, _)) => {
                        let response = read_http_request(&mut stream)
                            .and_then(|request| codec_response(&request))
                            .unwrap_or_else(|error| {
                                eprintln!("disposable Codec Server rejected a request: {error}");
                                (
                                    "400 Bad Request",
                                    serde_json::json!({"error": error}).to_string(),
                                )
                            });
                        write_http_response(&mut stream, response.0, &response.1);
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                    }
                    Err(error) => panic!("Codec Server accept failed: {error}"),
                }
            }
        });
        Self {
            address: format!("http://{address}"),
            stop,
            thread: Some(thread),
        }
    }

    fn endpoint(&self) -> String {
        format!("{}/{{namespace}}", self.address)
    }
}

impl Drop for CodecServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.join().expect("join Codec Server");
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires the project-local Temporal CLI"]
async fn live_dashboard_and_control_operations() {
    Box::pin(tokio::task::LocalSet::new().run_until(run_live_dashboard_and_control_operations()))
        .await;
}

async fn run_live_dashboard_and_control_operations() {
    let temporal_cli = temporal_cli();
    assert!(
        temporal_cli.is_file(),
        "install Temporal CLI first: scripts/install-temporal-cli.sh"
    );
    let port = free_port();
    let address = format!("127.0.0.1:{port}");
    let _server = start_dev_server(&temporal_cli, port);
    let service = connect_with_retry(&address).await;

    let cluster = service.cluster_info().await.expect("cluster info");
    assert!(!cluster.server_version.is_empty());
    assert!(
        service
            .list_namespaces()
            .await
            .expect("namespaces")
            .iter()
            .any(|namespace| namespace.name == "default")
    );

    let suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let search_attribute_name = format!("TuiContract{suffix}");
    service
        .add_search_attribute("default", &search_attribute_name, "Keyword")
        .await
        .expect("register Search Attribute");
    let attributes = service
        .list_search_attributes("default")
        .await
        .expect("list Search Attributes");
    assert!(attributes.iter().any(|attribute| {
        attribute.name == search_attribute_name
            && attribute.custom
            && attribute.value_type == "KEYWORD"
    }));
    service
        .remove_search_attribute("default", &search_attribute_name)
        .await
        .expect("remove Search Attribute");
    assert!(
        service
            .list_search_attributes("default")
            .await
            .expect("list Search Attributes after removal")
            .iter()
            .all(|attribute| attribute.name != search_attribute_name)
    );
    let control_workflow_id = format!("temporal-tui-control-{suffix}");
    let control_task_queue = format!("temporal-tui-control-{suffix}");
    let control_deployment = format!("temporal-tui-deployment-{suffix}");
    let control_build_v1 = format!("v1-{suffix}");
    let control_build_v2 = format!("v2-{suffix}");
    let worker_runtime =
        CoreRuntime::new_assume_tokio(RuntimeOptions::builder().build().expect("runtime options"))
            .expect("Temporal Core runtime");
    let worker_connection = Connection::connect(
        ConnectionOptions::new(
            url::Url::parse(&format!("http://{address}")).expect("worker address"),
        )
        .build(),
    )
    .await
    .expect("worker connection");
    let worker_client = Client::new(
        worker_connection,
        ClientOptions::new("default".to_string()).build(),
    )
    .expect("worker client");
    let worker_v1_options = WorkerOptions::new(control_task_queue.clone())
        .register_workflow::<TemporalTuiControlWorkflow>()
        .expect("register control Workflow")
        .task_types(WorkerTaskTypes::workflow_only())
        .deployment_options(WorkerDeploymentOptions {
            version: WorkerDeploymentVersion {
                deployment_name: control_deployment.clone(),
                build_id: control_build_v1.clone(),
            },
            use_worker_versioning: true,
            default_versioning_behavior: Some(VersioningBehavior::AutoUpgrade),
        })
        .build();
    let worker_v2_options = WorkerOptions::new(control_task_queue.clone())
        .register_workflow::<TemporalTuiControlWorkflow>()
        .expect("register second control Workflow")
        .task_types(WorkerTaskTypes::workflow_only())
        .deployment_options(WorkerDeploymentOptions {
            version: WorkerDeploymentVersion {
                deployment_name: control_deployment.clone(),
                build_id: control_build_v2.clone(),
            },
            use_worker_versioning: true,
            default_versioning_behavior: Some(VersioningBehavior::AutoUpgrade),
        })
        .build();
    let mut worker_v1 = Worker::new(&worker_runtime, worker_client.clone(), worker_v1_options)
        .expect("control Worker v1");
    let mut worker_v2 =
        Worker::new(&worker_runtime, worker_client, worker_v2_options).expect("control Worker v2");
    let shutdown_worker_v1 = worker_v1.shutdown_handle();
    let shutdown_worker_v2 = worker_v2.shutdown_handle();
    let worker_v1_task = tokio::task::spawn_local(async move { Box::pin(worker_v1.run()).await });
    let worker_v2_task = tokio::task::spawn_local(async move { Box::pin(worker_v2.run()).await });

    eventually(Duration::from_secs(10), || async {
        service
            .describe_worker_deployment("default", &control_deployment)
            .await
            .ok()
            .filter(|deployment| deployment.versions.len() >= 2)
    })
    .await
    .expect("both Worker Deployment versions should register");
    let capabilities = service
        .server_capabilities("default")
        .await
        .expect("negotiate server capabilities");
    assert_eq!(capabilities.namespace, "default");
    assert!(!capabilities.server_version.is_empty());
    for expected in [
        Capability::WorkflowUpdate,
        Capability::WorkflowPause,
        Capability::Schedules,
        Capability::WorkerHeartbeats,
        Capability::WorkerDeployments,
        Capability::BatchOperations,
        Capability::SearchAttributes,
    ] {
        let negotiated = capabilities
            .get(expected)
            .unwrap_or_else(|| panic!("missing negotiated capability: {}", expected.label()));
        assert_eq!(
            negotiated.availability,
            CapabilityAvailability::Available,
            "{} should be available: {}",
            expected.label(),
            negotiated.detail
        );
    }
    service
        .set_worker_deployment_current_version("default", &control_deployment, &control_build_v1)
        .await
        .expect("set current Worker Deployment version");
    service
        .set_worker_deployment_ramping_version(
            "default",
            &control_deployment,
            &control_build_v2,
            25.0,
        )
        .await
        .expect("set ramping Worker Deployment version");
    let ramped = service
        .describe_worker_deployment("default", &control_deployment)
        .await
        .expect("describe ramping Worker Deployment");
    assert_eq!(
        ramped
            .summary
            .current_version
            .as_ref()
            .map(|value| value.build_id.as_str()),
        Some(control_build_v1.as_str())
    );
    assert_eq!(
        ramped
            .summary
            .ramping_version
            .as_ref()
            .map(|value| value.build_id.as_str()),
        Some(control_build_v2.as_str())
    );
    assert!((ramped.summary.ramping_percentage - 25.0).abs() < f32::EPSILON);
    service
        .set_worker_deployment_current_version("default", &control_deployment, &control_build_v2)
        .await
        .expect("promote ramping Worker Deployment version");
    service
        .set_worker_deployment_ramping_version("default", &control_deployment, "", 0.0)
        .await
        .expect("clear ramping Worker Deployment version");
    let promoted = service
        .describe_worker_deployment("default", &control_deployment)
        .await
        .expect("describe promoted Worker Deployment");
    assert_eq!(
        promoted
            .summary
            .current_version
            .as_ref()
            .map(|value| value.build_id.as_str()),
        Some(control_build_v2.as_str())
    );
    assert!(promoted.summary.ramping_version.is_none());
    start_control_workflow(
        &temporal_cli,
        &address,
        &control_workflow_id,
        &control_task_queue,
    );

    let control_query = format!("WorkflowId = '{control_workflow_id}'");
    let control_workflow = eventually(Duration::from_secs(10), || async {
        service
            .list_workflows("default", &control_query, 10, Vec::new())
            .await
            .ok()
            .and_then(|page| page.workflows.into_iter().next())
            .filter(|workflow| workflow.status == WorkflowStatus::Running)
    })
    .await
    .expect("control Workflow should be running");
    let initial_query = service
        .query_workflow("default", &control_workflow.key, "current", Vec::new())
        .await
        .expect("Workflow Query");
    assert_eq!(initial_query.fields[0].value, "0");
    let update = service
        .update_workflow(
            "default",
            &control_workflow.key,
            "set",
            vec![serde_json::json!(41)],
        )
        .await
        .expect("Workflow Update");
    assert_eq!(update.fields[0].value, "0");
    assert!(update.update_id.is_some());
    let updated_query = service
        .query_workflow("default", &control_workflow.key, "current", Vec::new())
        .await
        .expect("Query updated Workflow state");
    assert_eq!(updated_query.fields[0].value, "41");

    service
        .pause_workflow("default", &control_workflow.key, "live contract pause")
        .await
        .expect("pause Workflow");
    let paused = eventually(Duration::from_secs(10), || async {
        service
            .list_workflows("default", &control_query, 10, Vec::new())
            .await
            .ok()
            .and_then(|page| page.workflows.into_iter().next())
            .filter(|workflow| workflow.status == WorkflowStatus::Paused)
    })
    .await;
    assert!(
        paused.is_some(),
        "Workflow should enter PAUSED visibility state"
    );
    service
        .unpause_workflow("default", &control_workflow.key, "live contract unpause")
        .await
        .expect("unpause Workflow");

    let control_details = eventually(Duration::from_secs(10), || async {
        service
            .describe_workflow("default", &control_workflow.key)
            .await
            .ok()
            .filter(|details| {
                details
                    .events
                    .iter()
                    .any(|event| event.event_type == "WORKFLOW TASK COMPLETED")
            })
    })
    .await
    .expect("control Workflow should have a completed Workflow Task");
    let reset_event_id = control_details
        .events
        .iter()
        .find(|event| event.event_type == "WORKFLOW TASK COMPLETED")
        .expect("reset boundary")
        .event_id;
    let reset_run_id = service
        .reset_workflow(
            "default",
            &control_workflow.key,
            reset_event_id,
            "live contract reset",
        )
        .await
        .expect("reset Workflow");
    assert!(!reset_run_id.is_empty());
    assert_ne!(reset_run_id, control_workflow.key.run_id);
    let reset_key = WorkflowKey {
        workflow_id: control_workflow_id.clone(),
        run_id: reset_run_id,
    };
    let reset_query = eventually(Duration::from_secs(10), || async {
        service
            .query_workflow("default", &reset_key, "current", Vec::new())
            .await
            .ok()
    })
    .await
    .expect("query reset Workflow");
    assert_eq!(reset_query.fields[0].value, "0");

    let workflow_id = format!("temporal-tui-smoke-{suffix}");
    let second_workflow_id = format!("temporal-tui-smoke-{suffix}-second");
    start_smoke_workflow(&temporal_cli, &address, &workflow_id);
    start_smoke_workflow(&temporal_cli, &address, &second_workflow_id);

    let type_query = "WorkflowType = 'TemporalTuiSmokeWorkflow'";
    let count = eventually(Duration::from_secs(10), || async {
        service
            .count_workflows("default", type_query)
            .await
            .ok()
            .filter(|count| count.total >= 2)
    })
    .await
    .expect("visibility count should include both smoke workflows");
    assert!(count.total >= 2);
    let first_page = service
        .list_workflows("default", type_query, 1, Vec::new())
        .await
        .expect("first cursor page");
    assert_eq!(first_page.workflows.len(), 1);
    assert!(!first_page.next_page_token.is_empty());
    let first_key = first_page.workflows[0].key.clone();
    let second_page = service
        .list_workflows("default", type_query, 1, first_page.next_page_token)
        .await
        .expect("second cursor page");
    assert_eq!(second_page.workflows.len(), 1);
    assert_ne!(first_key, second_page.workflows[0].key);

    let batch_workflow_type = format!("TemporalTuiBatchWorkflow{suffix}");
    let batch_task_queue = format!("temporal-tui-batch-{suffix}");
    for index in 0..4 {
        start_unpolled_workflow(
            &temporal_cli,
            &address,
            &format!("temporal-tui-batch-{suffix}-{index}"),
            &batch_workflow_type,
            &batch_task_queue,
        );
    }
    let batch_query =
        format!("WorkflowType = '{batch_workflow_type}' AND ExecutionStatus = 'Running'");
    let batch_preview = eventually(Duration::from_secs(10), || async {
        service
            .count_workflows("default", &batch_query)
            .await
            .ok()
            .filter(|count| count.total == 4)
    })
    .await
    .expect("Batch Operation preview count");
    assert_eq!(batch_preview.total, 4);
    let cancel_batch_job_id = format!("temporal-tui-cancel-batch-{suffix}");
    service
        .start_batch_operation(
            "default",
            BatchOperationRequest {
                job_id: cancel_batch_job_id.clone(),
                visibility_query: batch_query,
                reason: "live contract Batch cancellation".to_string(),
                max_operations_per_second: 100.0,
                kind: BatchOperationKind::Cancel,
                signal_name: String::new(),
                signal_input: serde_json::Value::Null,
            },
        )
        .await
        .expect("start Batch cancellation");
    let completed_batch = eventually(Duration::from_secs(20), || async {
        service
            .describe_batch_operation("default", &cancel_batch_job_id)
            .await
            .ok()
            .filter(|details| details.summary.state == "COMPLETED")
    })
    .await
    .expect("Batch cancellation should complete");
    assert_eq!(completed_batch.operation_type, "CANCEL");
    assert_eq!(completed_batch.total_operation_count, 4);
    assert_eq!(completed_batch.complete_operation_count, 4);
    assert_eq!(completed_batch.failure_operation_count, 0);

    let stopped_batch_workflow_type = format!("TemporalTuiStoppedBatchWorkflow{suffix}");
    let stopped_batch_task_queue = format!("temporal-tui-stopped-batch-{suffix}");
    for index in 0..8 {
        start_unpolled_workflow(
            &temporal_cli,
            &address,
            &format!("temporal-tui-stopped-batch-{suffix}-{index}"),
            &stopped_batch_workflow_type,
            &stopped_batch_task_queue,
        );
    }
    let stopped_batch_query =
        format!("WorkflowType = '{stopped_batch_workflow_type}' AND ExecutionStatus = 'Running'");
    eventually(Duration::from_secs(10), || async {
        service
            .count_workflows("default", &stopped_batch_query)
            .await
            .ok()
            .filter(|count| count.total == 8)
    })
    .await
    .expect("stoppable Batch Operation preview count");
    let stopped_batch_job_id = format!("temporal-tui-stopped-batch-{suffix}");
    service
        .start_batch_operation(
            "default",
            BatchOperationRequest {
                job_id: stopped_batch_job_id.clone(),
                visibility_query: stopped_batch_query,
                reason: "live contract stoppable Batch termination".to_string(),
                max_operations_per_second: 0.1,
                kind: BatchOperationKind::Terminate,
                signal_name: String::new(),
                signal_input: serde_json::Value::Null,
            },
        )
        .await
        .expect("start stoppable Batch termination");
    eventually(Duration::from_secs(10), || async {
        service
            .describe_batch_operation("default", &stopped_batch_job_id)
            .await
            .ok()
            .filter(|details| details.summary.state == "RUNNING")
    })
    .await
    .expect("Batch termination should enter RUNNING state");
    service
        .stop_batch_operation("default", &stopped_batch_job_id, "live contract stop")
        .await
        .expect("stop Batch termination");
    let stopped_batch = eventually(Duration::from_secs(10), || async {
        service
            .describe_batch_operation("default", &stopped_batch_job_id)
            .await
            .ok()
            .filter(|details| details.summary.state != "RUNNING")
    })
    .await
    .expect("stopped Batch termination should leave RUNNING state");
    assert_eq!(stopped_batch.operation_type, "TERMINATE");
    assert_eq!(stopped_batch.summary.state, "FAILED");
    assert!(stopped_batch.summary.close_time.is_some());

    let first_batch_page = service
        .list_batch_operations("default", 1, Vec::new())
        .await
        .expect("first Batch Operation cursor page");
    assert_eq!(first_batch_page.operations.len(), 1);
    assert!(!first_batch_page.next_page_token.is_empty());
    let second_batch_page = service
        .list_batch_operations("default", 1, first_batch_page.next_page_token)
        .await
        .expect("second Batch Operation cursor page");
    assert_eq!(second_batch_page.operations.len(), 1);
    assert_ne!(
        first_batch_page.operations[0].job_id,
        second_batch_page.operations[0].job_id
    );

    let query = format!("WorkflowId = '{workflow_id}'");
    let workflow = eventually(Duration::from_secs(10), || async {
        service
            .list_workflows("default", &query, 10, Vec::new())
            .await
            .ok()
            .and_then(|page| page.workflows.into_iter().next())
    })
    .await
    .expect("started workflow should appear in visibility");
    assert_eq!(workflow.status, WorkflowStatus::Running);

    let task_queues = eventually(Duration::from_secs(10), || async {
        service
            .list_task_queues("default", vec!["temporal-tui-smoke".to_string()])
            .await
            .ok()
            .filter(|queues| {
                queues.len() == 2
                    && queues.iter().any(|queue| {
                        queue.queue_type.label() == "WORKFLOW"
                            && queue.stats.approximate_backlog_count >= 2
                    })
            })
    })
    .await
    .expect("Task Queue diagnostics should expose the queued Workflow Tasks");
    assert!(task_queues.iter().any(|queue| {
        queue.queue_type.label() == "WORKFLOW"
            && queue.pollers.is_empty()
            && queue.stats.approximate_backlog_count >= 2
    }));
    assert!(
        task_queues
            .iter()
            .any(|queue| queue.queue_type.label() == "ACTIVITY")
    );

    let workers = eventually(Duration::from_secs(10), || async {
        service
            .list_workers("default", "", 10, Vec::new())
            .await
            .ok()
            .filter(|page| {
                page.workers
                    .iter()
                    .any(|worker| worker.task_queue == control_task_queue)
            })
    })
    .await
    .expect("experimental Worker observability endpoint");
    assert!(
        workers
            .workers
            .iter()
            .any(|worker| worker.task_queue == control_task_queue)
    );
    assert!(workers.next_page_token.is_empty());

    let deployments = service
        .list_worker_deployments("default", 10, Vec::new())
        .await
        .expect("GA Worker Deployment endpoint");
    assert!(deployments.deployments.iter().any(|deployment| {
        deployment.name == control_deployment
            && deployment
                .current_version
                .as_ref()
                .is_some_and(|version| version.build_id == control_build_v2)
            && deployment.ramping_version.is_none()
    }));
    assert!(deployments.next_page_token.is_empty());

    let chain = service
        .list_workflow_chain("default", &workflow_id)
        .await
        .expect("workflow chain");
    assert_eq!(chain.len(), 1);
    assert_eq!(chain[0].key.workflow_id, workflow_id);

    let details = service
        .describe_workflow("default", &workflow.key)
        .await
        .expect("workflow details");
    assert_eq!(details.summary.key.workflow_id, workflow_id);
    assert!(
        details
            .events
            .iter()
            .any(|event| event.event_type == "WORKFLOW EXECUTION STARTED")
    );

    service
        .signal_workflow(
            "default",
            &workflow.key,
            "smoke-signal",
            serde_json::json!({"approved": true}),
        )
        .await
        .expect("signal workflow");
    let signaled = eventually(Duration::from_secs(5), || async {
        service
            .describe_workflow("default", &workflow.key)
            .await
            .ok()
            .filter(|details| {
                details.events.iter().any(|event| {
                    event.event_type == "WORKFLOW EXECUTION SIGNALED"
                        && event.detail == "smoke-signal"
                })
            })
    })
    .await;
    assert!(
        signaled.is_some(),
        "signal event should be visible in history"
    );

    let codec_server = CodecServer::start();
    let codec_service = GrpcTemporalService::connect(TemporalConnectionConfig {
        address: address.clone(),
        api_key: None,
        headers: HashMap::new(),
        tls: None,
        payload_codec: Some(PayloadCodecConfig {
            endpoint: codec_server.endpoint(),
            headers: HashMap::new(),
        }),
    })
    .await
    .expect("connect client with Codec Server");
    codec_service
        .signal_workflow(
            "default",
            &workflow.key,
            "codec-signal",
            serde_json::json!({"codec_value":"roundtrip"}),
        )
        .await
        .expect("send encoded signal");
    let encoded = eventually(Duration::from_secs(5), || async {
        service
            .describe_workflow("default", &workflow.key)
            .await
            .ok()
            .and_then(|details| {
                details
                    .events
                    .into_iter()
                    .find(|event| event.detail == "codec-signal")
            })
    })
    .await
    .expect("encoded signal event");
    assert!(encoded.fields.iter().any(|field| {
        field.encoding == "binary/encrypted" && !field.value.contains("roundtrip")
    }));
    let decoded = codec_service
        .describe_workflow("default", &workflow.key)
        .await
        .expect("decode Workflow history through Codec Server");
    let decoded = decoded
        .events
        .iter()
        .find(|event| event.detail == "codec-signal")
        .expect("decoded signal event");
    assert!(
        decoded
            .fields
            .iter()
            .any(|field| { field.encoding == "json/plain" && field.value.contains("roundtrip") })
    );

    let schedule_id = format!("temporal-tui-schedule-{suffix}");
    let second_schedule_id = format!("temporal-tui-schedule-{suffix}-second");
    let scheduled_workflow_type = format!("TemporalTuiScheduledWorkflow{suffix}");
    codec_service
        .create_schedule(
            "default",
            ScheduleCreateRequest {
                schedule_id: schedule_id.clone(),
                workflow_id: format!("temporal-tui-scheduled-{suffix}"),
                workflow_type: scheduled_workflow_type.clone(),
                task_queue: "temporal-tui-scheduled".to_string(),
                schedule_expression: "@every 1h".to_string(),
                timezone: "UTC".to_string(),
                arguments: vec![serde_json::json!({
                    "codec_value":"scheduled-roundtrip"
                })],
                paused: false,
                notes: "live contract".to_string(),
            },
        )
        .await
        .expect("create encoded Schedule");
    service
        .create_schedule(
            "default",
            ScheduleCreateRequest {
                schedule_id: second_schedule_id.clone(),
                workflow_id: format!("temporal-tui-scheduled-{suffix}-second"),
                workflow_type: scheduled_workflow_type.clone(),
                task_queue: "temporal-tui-scheduled".to_string(),
                schedule_expression: "@every 1h".to_string(),
                timezone: "UTC".to_string(),
                arguments: vec![serde_json::json!({"source":"pagination"})],
                paused: true,
                notes: "second live contract Schedule".to_string(),
            },
        )
        .await
        .expect("create second Schedule");

    let schedules = eventually(Duration::from_secs(10), || async {
        service
            .list_schedules("default", "", 100, Vec::new())
            .await
            .ok()
            .filter(|page| {
                page.schedules
                    .iter()
                    .any(|schedule| schedule.schedule_id == schedule_id)
                    && page
                        .schedules
                        .iter()
                        .any(|schedule| schedule.schedule_id == second_schedule_id)
            })
    })
    .await
    .expect("Schedules should appear in visibility");
    assert!(schedules.schedules.len() >= 2);
    let first_schedule_page = service
        .list_schedules("default", "", 1, Vec::new())
        .await
        .expect("first Schedule cursor page");
    assert_eq!(first_schedule_page.schedules.len(), 1);
    assert!(!first_schedule_page.next_page_token.is_empty());
    let second_schedule_page = service
        .list_schedules("default", "", 1, first_schedule_page.next_page_token)
        .await
        .expect("second Schedule cursor page");
    assert_eq!(second_schedule_page.schedules.len(), 1);
    assert_ne!(
        first_schedule_page.schedules[0].schedule_id,
        second_schedule_page.schedules[0].schedule_id
    );

    let encoded_schedule = service
        .describe_schedule("default", &schedule_id)
        .await
        .expect("describe encoded Schedule without Codec Server");
    assert!(encoded_schedule.input.iter().any(|field| {
        field.encoding == "binary/encrypted" && !field.value.contains("scheduled-roundtrip")
    }));
    let decoded_schedule = codec_service
        .describe_schedule("default", &schedule_id)
        .await
        .expect("decode Schedule input through Codec Server");
    assert!(decoded_schedule.input.iter().any(|field| {
        field.encoding == "json/plain" && field.value.contains("scheduled-roundtrip")
    }));

    codec_service
        .pause_schedule("default", &schedule_id, "live contract pause")
        .await
        .expect("pause Schedule");
    let paused_schedule = eventually(Duration::from_secs(10), || async {
        codec_service
            .describe_schedule("default", &schedule_id)
            .await
            .ok()
            .filter(|schedule| schedule.summary.paused)
    })
    .await;
    assert!(paused_schedule.is_some(), "Schedule should be paused");
    codec_service
        .unpause_schedule("default", &schedule_id, "live contract unpause")
        .await
        .expect("unpause Schedule");
    codec_service
        .update_schedule(
            "default",
            &schedule_id,
            ScheduleUpdateRequest {
                schedule_expression: Some("@every 2h".to_string()),
                timezone: Some("UTC".to_string()),
                notes: "updated with conflict token".to_string(),
            },
        )
        .await
        .expect("update Schedule definition");
    let updated_schedule = codec_service
        .describe_schedule("default", &schedule_id)
        .await
        .expect("describe updated Schedule");
    assert_eq!(
        updated_schedule.summary.notes,
        "updated with conflict token"
    );
    assert!(
        updated_schedule
            .timing
            .iter()
            .any(|timing| timing == "every 2h")
    );
    assert!(
        updated_schedule
            .input
            .iter()
            .any(|field| field.value.contains("scheduled-roundtrip")),
        "Schedule update must preserve the encoded action input"
    );

    codec_service
        .trigger_schedule("default", &schedule_id)
        .await
        .expect("trigger Schedule");
    let triggered = eventually(Duration::from_secs(10), || async {
        codec_service
            .describe_schedule("default", &schedule_id)
            .await
            .ok()
            .filter(|schedule| schedule.action_count >= 1)
    })
    .await;
    assert!(
        triggered.is_some(),
        "Schedule trigger should record an action"
    );
    let backfill_end = chrono::Utc::now();
    codec_service
        .backfill_schedule(
            "default",
            &schedule_id,
            ScheduleBackfillRequest {
                start_time: backfill_end - chrono::Duration::hours(6),
                end_time: backfill_end,
                overlap_policy: "allow-all".to_string(),
            },
        )
        .await
        .expect("backfill Schedule");
    let backfilled = eventually(Duration::from_secs(10), || async {
        codec_service
            .describe_schedule("default", &schedule_id)
            .await
            .ok()
            .filter(|schedule| schedule.action_count >= 2)
    })
    .await;
    assert!(
        backfilled.is_some(),
        "Schedule backfill should record additional actions"
    );

    codec_service
        .delete_schedule("default", &schedule_id)
        .await
        .expect("delete Schedule");
    service
        .delete_schedule("default", &second_schedule_id)
        .await
        .expect("delete second Schedule");
    let schedules_deleted = eventually(Duration::from_secs(10), || async {
        service
            .list_schedules("default", "", 100, Vec::new())
            .await
            .ok()
            .filter(|page| {
                !page.schedules.iter().any(|schedule| {
                    schedule.schedule_id == schedule_id
                        || schedule.schedule_id == second_schedule_id
                })
            })
    })
    .await;
    assert!(
        schedules_deleted.is_some(),
        "deleted Schedules should disappear from visibility"
    );

    service
        .signal_workflow(
            "default",
            &workflow.key,
            "redaction-signal",
            serde_json::json!({"customer":"Ada","credentials":{"api_key":"never-export"}}),
        )
        .await
        .expect("signal sensitive payload");
    for index in 0..205 {
        service
            .signal_workflow(
                "default",
                &workflow.key,
                "pagination-signal",
                serde_json::json!({"index": index}),
            )
            .await
            .expect("signal for history pagination");
    }

    let first_history_page = service
        .describe_workflow("default", &workflow.key)
        .await
        .expect("paginated workflow details");
    assert!(
        !first_history_page.history_next_page_token.is_empty(),
        "history over 200 events should expose another cursor"
    );
    let mut all_events = first_history_page.events;
    let mut history_token = first_history_page.history_next_page_token;
    while !history_token.is_empty() {
        let page = service
            .load_history_page("default", &workflow.key, history_token)
            .await
            .expect("older history page");
        all_events.extend(page.events);
        history_token = page.next_page_token;
    }
    all_events.sort_by_key(|event| event.event_id);
    all_events.dedup_by_key(|event| event.event_id);
    assert!(
        all_events
            .iter()
            .any(|event| event.event_type == "WORKFLOW EXECUTION STARTED")
    );
    let redacted_event = all_events
        .iter()
        .find(|event| event.detail == "redaction-signal")
        .expect("redaction signal event");
    assert!(redacted_event.fields.iter().any(|field| {
        field.redacted
            && field.value.contains("<redacted>")
            && !field.value.contains("never-export")
    }));

    service
        .cancel_workflow("default", &workflow.key, "live contract test")
        .await
        .expect("request cancellation");
    let cancel_requested = eventually(Duration::from_secs(5), || async {
        service
            .describe_workflow("default", &workflow.key)
            .await
            .ok()
            .filter(|details| {
                details
                    .events
                    .iter()
                    .any(|event| event.event_type == "WORKFLOW EXECUTION CANCEL REQUESTED")
            })
    })
    .await;
    assert!(
        cancel_requested.is_some(),
        "cancellation request should be visible in history"
    );

    service
        .terminate_workflow("default", &workflow.key, "live contract test cleanup")
        .await
        .expect("terminate workflow");
    let terminated = eventually(Duration::from_secs(10), || async {
        service
            .list_workflows("default", &query, 10, Vec::new())
            .await
            .ok()
            .and_then(|page| page.workflows.into_iter().next())
            .filter(|workflow| workflow.status == WorkflowStatus::Terminated)
    })
    .await;
    assert!(
        terminated.is_some(),
        "terminated workflow should reach terminal visibility state"
    );

    let second = service
        .list_workflows(
            "default",
            &format!("WorkflowId = '{second_workflow_id}'"),
            10,
            Vec::new(),
        )
        .await
        .expect("second workflow visibility")
        .workflows
        .into_iter()
        .next()
        .expect("second workflow");
    service
        .terminate_workflow("default", &second.key, "live contract test cleanup")
        .await
        .expect("terminate second workflow");

    service
        .terminate_workflow("default", &reset_key, "live contract control cleanup")
        .await
        .expect("terminate reset control Workflow");
    let scheduled_query = format!("WorkflowType = '{scheduled_workflow_type}'");
    let scheduled_workflows = eventually(Duration::from_secs(10), || async {
        service
            .list_workflows("default", &scheduled_query, 100, Vec::new())
            .await
            .ok()
            .filter(|page| !page.workflows.is_empty())
    })
    .await
    .expect("Schedule-triggered Workflows should appear in visibility");
    for scheduled_workflow in scheduled_workflows.workflows {
        if scheduled_workflow.status.is_running() {
            service
                .terminate_workflow(
                    "default",
                    &scheduled_workflow.key,
                    "live contract Schedule cleanup",
                )
                .await
                .expect("terminate Schedule-triggered Workflow");
        }
    }
    for batch_type in [&batch_workflow_type, &stopped_batch_workflow_type] {
        let batch_workflows = service
            .list_workflows(
                "default",
                &format!("WorkflowType = '{batch_type}'"),
                100,
                Vec::new(),
            )
            .await
            .expect("list Batch Operation cleanup Workflows");
        for batch_workflow in batch_workflows.workflows {
            if batch_workflow.status.is_running() {
                service
                    .terminate_workflow(
                        "default",
                        &batch_workflow.key,
                        "live contract Batch Operation cleanup",
                    )
                    .await
                    .expect("terminate Batch Operation cleanup Workflow");
            }
        }
    }

    shutdown_worker_v1();
    shutdown_worker_v2();
    let worker_v1_result = tokio::time::timeout(Duration::from_secs(10), worker_v1_task)
        .await
        .expect("control Worker v1 shutdown timeout")
        .expect("join control Worker v1");
    worker_v1_result.expect("control Worker v1 result");
    let worker_v2_result = tokio::time::timeout(Duration::from_secs(10), worker_v2_task)
        .await
        .expect("control Worker v2 shutdown timeout")
        .expect("join control Worker v2");
    worker_v2_result.expect("control Worker v2 result");
}

fn temporal_cli() -> PathBuf {
    std::env::var_os("TEMPORAL_CLI").map_or_else(
        || {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join(".tools")
                .join("bin")
                .join("temporal")
        },
        PathBuf::from,
    )
}

fn free_port() -> u16 {
    TcpListener::bind(("127.0.0.1", 0))
        .expect("bind ephemeral port")
        .local_addr()
        .expect("ephemeral local address")
        .port()
}

fn start_dev_server(temporal_cli: &PathBuf, port: u16) -> DevServer {
    let port = port.to_string();
    let child = Command::new(temporal_cli)
        .args([
            "server",
            "start-dev",
            "--headless",
            "--ip",
            "127.0.0.1",
            "--port",
            &port,
            "--dynamic-config-value",
            "frontend.WorkflowPauseEnabled=true",
            "--log-level",
            "error",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start Temporal dev server");
    DevServer { child }
}

async fn connect_with_retry(address: &str) -> GrpcTemporalService {
    let mut last_error = None;
    for _ in 0..80 {
        match GrpcTemporalService::connect(TemporalConnectionConfig {
            address: address.to_string(),
            api_key: None,
            headers: HashMap::new(),
            tls: None,
            payload_codec: None,
        })
        .await
        {
            Ok(service) => return service,
            Err(error) => last_error = Some(error),
        }
        sleep(Duration::from_millis(100)).await;
    }
    panic!(
        "Temporal dev server did not become ready: {}",
        last_error.unwrap()
    );
}

fn run_cli(temporal_cli: &PathBuf, arguments: &[&str]) {
    let output = Command::new(temporal_cli)
        .args(arguments)
        .output()
        .expect("run Temporal CLI");
    assert!(
        output.status.success(),
        "Temporal CLI failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn start_smoke_workflow(temporal_cli: &PathBuf, address: &str, workflow_id: &str) {
    start_unpolled_workflow(
        temporal_cli,
        address,
        workflow_id,
        "TemporalTuiSmokeWorkflow",
        "temporal-tui-smoke",
    );
}

fn start_unpolled_workflow(
    temporal_cli: &PathBuf,
    address: &str,
    workflow_id: &str,
    workflow_type: &str,
    task_queue: &str,
) {
    run_cli(
        temporal_cli,
        &[
            "workflow",
            "start",
            "--address",
            address,
            "--namespace",
            "default",
            "--workflow-id",
            workflow_id,
            "--type",
            workflow_type,
            "--task-queue",
            task_queue,
            "--static-summary",
            "Temporal TUI live smoke test",
            "--input",
            r#"{"source":"temporal-tui"}"#,
            "--output",
            "none",
        ],
    );
}

fn start_control_workflow(
    temporal_cli: &PathBuf,
    address: &str,
    workflow_id: &str,
    task_queue: &str,
) {
    run_cli(
        temporal_cli,
        &[
            "workflow",
            "start",
            "--address",
            address,
            "--namespace",
            "default",
            "--workflow-id",
            workflow_id,
            "--type",
            "TemporalTuiControlWorkflow",
            "--task-queue",
            task_queue,
            "--input",
            "0",
            "--output",
            "none",
        ],
    );
}

fn read_http_request(stream: &mut std::net::TcpStream) -> Result<String, String> {
    stream
        .set_nonblocking(false)
        .map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .map_err(|error| error.to_string())?;
    let mut request = Vec::new();
    let mut buffer = [0_u8; 8192];
    let mut expected_length = None;
    loop {
        let count = stream
            .read(&mut buffer)
            .map_err(|error| error.to_string())?;
        if count == 0 {
            break;
        }
        request.extend_from_slice(&buffer[..count]);
        if expected_length.is_none()
            && let Some(header_end) = request.windows(4).position(|window| window == b"\r\n\r\n")
        {
            let headers = String::from_utf8_lossy(&request[..header_end]);
            let content_length = headers.lines().find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            });
            expected_length = content_length.map(|length| header_end.saturating_add(4 + length));
        }
        if expected_length.is_some_and(|length| request.len() >= length) {
            break;
        }
    }
    String::from_utf8(request).map_err(|error| error.to_string())
}

fn codec_response(request: &str) -> Result<(&'static str, String), String> {
    let (headers, body) = request
        .split_once("\r\n\r\n")
        .ok_or_else(|| "missing HTTP body".to_string())?;
    let operation = if headers.starts_with("POST ") && headers.contains("/encode HTTP/") {
        "encode"
    } else if headers.starts_with("POST ") && headers.contains("/decode HTTP/") {
        "decode"
    } else {
        return Err("unsupported Codec Server path".to_string());
    };
    let mut payloads: serde_json::Value =
        serde_json::from_str(body).map_err(|error| error.to_string())?;
    let payloads = payloads
        .get_mut("payloads")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or_else(|| "missing payloads array".to_string())?;
    for payload in &mut *payloads {
        if operation == "encode" {
            let raw = serde_json::to_vec(payload).map_err(|error| error.to_string())?;
            *payload = serde_json::json!({
                "metadata": {
                    "encoding": BASE64_STANDARD.encode("binary/encrypted")
                },
                "data": BASE64_STANDARD.encode(raw)
            });
            continue;
        }
        let encoding = payload
            .get("metadata")
            .and_then(|metadata| metadata.get("encoding"))
            .and_then(serde_json::Value::as_str)
            .and_then(|value| BASE64_STANDARD.decode(value).ok());
        if encoding.as_deref() == Some(b"binary/encrypted") {
            let data = payload
                .get("data")
                .and_then(serde_json::Value::as_str)
                .ok_or_else(|| "encrypted payload has no data".to_string())?;
            let data = BASE64_STANDARD
                .decode(data)
                .map_err(|error| error.to_string())?;
            *payload = serde_json::from_slice(&data).map_err(|error| error.to_string())?;
        }
    }
    Ok(("200 OK", payloads_to_body(payloads)))
}

fn payloads_to_body(payloads: &[serde_json::Value]) -> String {
    serde_json::json!({ "payloads": payloads }).to_string()
}

fn write_http_response(stream: &mut std::net::TcpStream, status: &str, body: &str) {
    write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .expect("write Codec Server response");
}

async fn eventually<T, FutureType, Factory>(timeout: Duration, mut factory: Factory) -> Option<T>
where
    FutureType: std::future::Future<Output = Option<T>>,
    Factory: FnMut() -> FutureType,
{
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Some(value) = factory().await {
            return Some(value);
        }
        if tokio::time::Instant::now() >= deadline {
            return None;
        }
        sleep(Duration::from_millis(100)).await;
    }
}
