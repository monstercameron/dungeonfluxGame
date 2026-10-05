//! Ignored operator-only generated-service recovery proof on Root-registered loopback PG.
//! No ordinary service route, public fault message or hosted authority is introduced.
use super::{Service, actor, hex, journey, model, wire};
use df_model::checkpoint::Checkpoint;
use df_persistence::local_demo_scope::{self, LocalDemoAuthority, LocalDemoScopeIssuer};
use df_persistence::{
    NativeCodecLimits, NativeRepositoryOptions, NativeTransactionBounds, PostgresRepository,
};
use df_protocol::common as rpc;
use df_session::inbox::{InboxHandle, bounded_inbox};
use df_session::submission::DurableOwner;
use prost::Message;
use sha2::{Digest, Sha256};
use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use tokio::{
    sync::{Semaphore, oneshot, watch},
    task::JoinHandle,
    time::{Instant, timeout},
};
use tokio_postgres::{Client, Config, NoTls};
use tonic::Request;

const REPORT: &str = "/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/engine-recovery-20261005/engine-recovery-qualification-report-04.json";
const COURIER_REPORT: &str = "/Users/earlcameron/Desktop/dungeonflux/artifacts/tmp/engine-courier-ai-20261005/courier-pending-restart-report-01.json";
type Error = Box<dyn std::error::Error + Send + Sync>;
#[track_caller]
fn required(value: bool) -> Result<(), Error> {
    if value {
        Ok(())
    } else {
        let caller = std::panic::Location::caller();
        Err(io::Error::other(format!(
            "recovery assertion {}:{}",
            caller.file(),
            caller.line()
        ))
        .into())
    }
}
fn configuration() -> Result<Config, Error> {
    required(
        std::env::var("DF_RECOVERY_QUALIFICATION").as_deref() == Ok("owned-loopback-recovery04"),
    )?;
    Ok(trusted_configuration(
        55517,
        "df-engine-recovery-inspector-01",
    ))
}
fn trusted_configuration(port: u16, application: &'static str) -> Config {
    let mut configuration = Config::new();
    configuration
        .host("127.0.0.1")
        .port(port)
        .dbname("df_gameplay_demo_20261005_recovery05")
        .user("df_gameplay_demo_admin_20261004")
        .ssl_mode(tokio_postgres::config::SslMode::Disable)
        .application_name(application);
    configuration
}
fn options(codec: NativeCodecLimits) -> Result<NativeRepositoryOptions, Error> {
    Ok(NativeRepositoryOptions {
        transaction_bounds: NativeTransactionBounds {
            transaction: Duration::from_secs(2),
            rollback: Duration::from_millis(250),
            driver_join: Duration::from_secs(2),
        },
        codec_limits: codec,
        maximum_receipt_bytes: 4096,
        verifier: Some(local_demo_scope::verifier()),
        recovery: Some(model::recovery().map_err(repository_error)?),
    })
}
async fn repository(
    configuration: &Config,
    reconnect: &Config,
    codec: NativeCodecLimits,
) -> Result<PostgresRepository<LocalDemoAuthority>, Error> {
    let (client, connection) =
        timeout(Duration::from_secs(2), configuration.connect(NoTls)).await??;
    let options = options(codec)?;
    let context = df_observe::OperationContext {
        trace_parent: String::new(),
        build: crate::BUILD_ID.to_owned(),
    };
    let mut repository = match PostgresRepository::from_connected_no_tls(
        tokio::runtime::Handle::current(),
        client,
        connection,
        options.clone(),
        &context,
    )
    .await
    {
        Ok(repository) => repository,
        Err(mut failure) => {
            let error = failure.error();
            failure.close().await.map_err(repository_error)?;
            return Err(io::Error::other(format!("recovery setup {error:?}")).into());
        }
    };
    repository
        .configure_reconnect(reconnect.clone(), options)
        .map_err(repository_error)?;
    Ok(repository)
}
struct ActorExit {
    checkpoint: Checkpoint,
    issuer: LocalDemoScopeIssuer,
}
struct RunningActor {
    service: Service,
    sender: InboxHandle<actor::Call>,
    thread: Option<std::thread::JoinHandle<Result<ActorExit, Error>>>,
}
impl RunningActor {
    fn start(
        repository: PostgresRepository<LocalDemoAuthority>,
        issuer: LocalDemoScopeIssuer,
        checkpoint: Checkpoint,
        codec: NativeCodecLimits,
        counter: Arc<AtomicUsize>,
    ) -> Result<Self, Error> {
        let (updates, receiver) = watch::channel(checkpoint.clone());
        let (publication, intent_notifications) = actor::Publication::new(updates.clone());
        let owner = DurableOwner::new(
            repository,
            actor::Engine(Some(counter)),
            publication,
            checkpoint,
            4096,
        )
        .map_err(repository_error)?;
        let (sender, inbox) = bounded_inbox::<actor::Call>();
        let thread = std::thread::Builder::new()
            .name("df-engine-recovery-owner".to_owned())
            .spawn(move || {
                let mut actor = actor::Actor {
                    owner,
                    bootstrap_credential: [0x81; 32],
                    issuer,
                    codec,
                    fenced: false,
                    recovery_wakeup: updates,
                    intent_notifications,
                    calls_remaining: 20,
                    qualification_inputs: Some(Vec::new()),
                    qualification_joins: Some(Vec::new()),
                };
                actor.run_committed_intents();
                let drained = inbox.run(&mut actor);
                let checkpoint = actor.owner.checkpoint().clone();
                let mut repository = actor.owner.into_repository();
                let first_close = repository.close();
                if first_close.is_err() {
                    repository.close().map_err(repository_error)?;
                }
                first_close.map_err(repository_error)?;
                drained.map_err(|_| io::Error::other("recovery actor drain failed"))?;
                Ok(ActorExit {
                    checkpoint,
                    issuer: actor.issuer,
                })
            })?;
        let service = Service {
            actor: sender.clone(),
            updates: receiver,
            streams: Arc::new(Semaphore::new(6)),
        };
        Ok(Self {
            service,
            sender,
            thread: Some(thread),
        })
    }
    async fn close(&mut self) -> Result<ActorExit, Error> {
        let stopped = self.sender.stop();
        let thread = self
            .thread
            .take()
            .ok_or_else(|| io::Error::other("actor join already consumed"))?;
        let joined = tokio::task::spawn_blocking(move || thread.join())
            .await?
            .map_err(|_| io::Error::other("recovery actor panicked"))?;
        stopped.map_err(|_| io::Error::other("recovery actor stop failed"))?;
        joined
    }
}
struct Calls {
    count: usize,
    deadline: Instant,
}
fn admit(calls: &mut Calls) -> Result<(), Error> {
    required(calls.count < 20 && Instant::now() < calls.deadline)?;
    calls.count = calls
        .count
        .checked_add(1)
        .ok_or_else(|| io::Error::other("fixture call overflow"))?;
    Ok(())
}
async fn join(service: &Service, operation: u8, calls: &mut Calls) -> Result<[u8; 32], Error> {
    admit(calls)?;
    let body = rpc::JoinRoomRequest {
        room_code: journey::ROOM_CODE.to_owned(),
        join_secret: vec![operation; 32],
        operation_id: Some(rpc::OperationId {
            value: Some(vec![operation; 16]),
        }),
    };
    let decoded = rpc::JoinRoomRequest::decode(body.encode_to_vec().as_slice())?;
    let response = rpc::room_service_server::RoomService::join(service, Request::new(decoded))
        .await?
        .into_inner();
    let Some(rpc::join_room_response::Outcome::Joined(joined)) = response.outcome else {
        return Err(io::Error::other("recovery real room join refused").into());
    };
    let bytes = joined.local_binding.as_bytes();
    required(bytes.len() == 64)?;
    let mut credential = [0; 32];
    for (index, pair) in bytes.as_chunks::<2>().0.iter().enumerate() {
        let digit = |b: u8| match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            _ => None,
        };
        credential[index] =
            digit(pair[0]).ok_or_else(|| io::Error::other("join grant encoding"))? * 16
                + digit(pair[1]).ok_or_else(|| io::Error::other("join grant encoding"))?;
    }
    Ok(credential)
}
fn create(checkpoint: &Checkpoint, operation: u8) -> rpc::SubmitActionRequest {
    let mut body = action(
        checkpoint,
        operation,
        rpc::GameplayActionKind::CreateCharacter,
    );
    body.character = Some(rpc::CharacterSelection {
        name: "Recovery-Fighter".to_owned(),
        choices: [
            ("species", "dwarf"),
            ("class", "fighter-1"),
            ("background", "soldier"),
            ("array", "stalwart"),
            ("alignment", "lawful-good"),
            ("language-1", "dwarvish"),
            ("language-2", "elvish"),
            ("skill-1", "perception"),
            ("skill-2", "survival"),
            ("gaming-set", "dice"),
            ("equipment", "fighter-a-soldier-a"),
            ("style-masteries", "defense-greatsword-flail-javelin"),
            ("allied-ties", "join-order"),
        ]
        .into_iter()
        .map(|(group, option)| rpc::JourneyChoice {
            group_id: group.to_owned(),
            option_id: option.to_owned(),
        })
        .collect(),
    });
    body
}
fn action(
    checkpoint: &Checkpoint,
    operation: u8,
    kind: rpc::GameplayActionKind,
) -> rpc::SubmitActionRequest {
    let basis = checkpoint.basis();
    rpc::SubmitActionRequest {
        session_id: Some(rpc::SessionId {
            value: Some(basis.session.as_bytes().to_vec()),
        }),
        run_id: Some(rpc::RunId {
            value: Some(basis.run.as_bytes().to_vec()),
        }),
        operation_id: Some(rpc::OperationId {
            value: Some(vec![operation; 16]),
        }),
        observed_revision: Some(wire::revision(basis.revision)),
        offer_id: journey::offer_id(checkpoint, kind),
        action_kind: kind as i32,
        ..Default::default()
    }
}
async fn submit(
    service: &Service,
    body: &rpc::SubmitActionRequest,
    credential: [u8; 32],
    calls: &mut Calls,
) -> Result<rpc::SubmitActionResponse, tonic::Status> {
    admit(calls).map_err(|_| tonic::Status::resource_exhausted("fixture call bound"))?;
    let decoded = rpc::SubmitActionRequest::decode(body.encode_to_vec().as_slice())
        .map_err(|_| tonic::Status::internal("fixture encoding"))?;
    let mut request = Request::new(decoded);
    request.metadata_mut().insert(
        "x-df-local-binding",
        hex(&credential)
            .parse()
            .map_err(|_| tonic::Status::internal("fixture metadata"))?,
    );
    rpc::action_service_server::ActionService::submit(service, request)
        .await
        .map(|value| value.into_inner())
}
fn committed(value: rpc::SubmitActionResponse) -> Result<rpc::DecisionReceipt, Error> {
    match value.outcome {
        Some(rpc::submit_action_response::Outcome::CommittedDecision(value)) => Ok(*value),
        _ => Err(io::Error::other("fixture receipt absent").into()),
    }
}
async fn snapshot(
    service: &Service,
    calls: &mut Calls,
) -> Result<actor::QualificationSnapshot, Error> {
    admit(calls)?;
    let (reply, wait) = oneshot::channel();
    service
        .actor
        .try_submit(actor::Call::QualificationInputs { reply })
        .map_err(|_| io::Error::other("fixture snapshot admission"))?;
    Ok(timeout(Duration::from_secs(5), wait).await???)
}
#[derive(Eq, PartialEq)]
struct Durable {
    counts: [i64; 4],
    operations: Vec<String>,
    facts: Vec<String>,
    intents: Vec<String>,
    envelope: Vec<u8>,
    receipt: Vec<u8>,
}
async fn durable(client: &Client, operation: u8) -> Result<Durable, Error> {
    let row=client.query_one("SELECT (SELECT count(*) FROM df_game.operations) AS operations,(SELECT count(*) FROM df_game.facts) AS facts,(SELECT count(*) FROM df_game.checkpoints) AS checkpoints,(SELECT count(*) FROM df_game.intents) AS intents,CASE WHEN octet_length(c.complete_envelope)<=1048576 THEN c.complete_envelope END AS envelope,CASE WHEN octet_length(o.receipt)<=4096 THEN o.receipt END AS receipt FROM df_game.sessions s JOIN df_game.checkpoints c ON (c.tenant_id,c.session_id,c.recovery_epoch,c.in_epoch_sequence)=(s.tenant_id,s.session_id,s.recovery_epoch,s.in_epoch_sequence) JOIN df_game.operations o ON o.tenant_id=s.tenant_id AND o.session_id=s.session_id WHERE s.tenant_id=$1 AND s.session_id=$2 AND o.operation_id=$3",&[&local_demo_scope::TENANT.as_slice(),&[0x41_u8;16].as_slice(),&[operation;16].as_slice()]).await?;
    Ok(Durable {
        counts: [
            row.try_get("operations")?,
            row.try_get("facts")?,
            row.try_get("checkpoints")?,
            row.try_get("intents")?,
        ],
        operations: physical_rows(client, "SELECT CASE WHEN octet_length(body)<=262144 AND sum(octet_length(body)) OVER ()<=1048576 THEN body END FROM (SELECT row_to_json(t)::text AS body,row_number() OVER (ORDER BY tenant_id,session_id,principal_id,command_namespace,recovery_epoch,operation_id) AS row_ordinal FROM df_game.operations t ORDER BY tenant_id,session_id,principal_id,command_namespace,recovery_epoch,operation_id LIMIT 257) bounded ORDER BY row_ordinal").await?,
        facts: physical_rows(client, "SELECT CASE WHEN octet_length(body)<=262144 AND sum(octet_length(body)) OVER ()<=1048576 THEN body END FROM (SELECT row_to_json(t)::text AS body,row_number() OVER (ORDER BY tenant_id,session_id,committed_epoch,committed_sequence,ordinal) AS row_ordinal FROM df_game.facts t ORDER BY tenant_id,session_id,committed_epoch,committed_sequence,ordinal LIMIT 257) bounded ORDER BY row_ordinal").await?,
        intents: physical_rows(client, "SELECT CASE WHEN octet_length(body)<=262144 AND sum(octet_length(body)) OVER ()<=1048576 THEN body END FROM (SELECT row_to_json(t)::text AS body,row_number() OVER (ORDER BY tenant_id,session_id,principal_id,command_namespace,recovery_epoch,operation_id,slot) AS row_ordinal FROM df_game.intents t ORDER BY tenant_id,session_id,principal_id,command_namespace,recovery_epoch,operation_id,slot LIMIT 257) bounded ORDER BY row_ordinal").await?,
        envelope: row.try_get("envelope")?,
        receipt: row.try_get("receipt")?,
    })
}
// Compare complete physical rows, including encoded payload bytes, without logging them.
// The SQL caps returned bytes before transfer; the extra row detects row-cap overflow.
async fn physical_rows(client: &Client, sql: &'static str) -> Result<Vec<String>, Error> {
    let rows = client.query(sql, &[]).await?;
    required(rows.len() <= 256)?;
    rows.into_iter()
        .map(|row| row.try_get(0).map_err(Into::into))
        .collect()
}
async fn join_driver(
    mut driver: JoinHandle<Result<(), tokio_postgres::Error>>,
) -> Result<(), Error> {
    match timeout(Duration::from_secs(2), &mut driver).await {
        Ok(joined) => {
            joined??;
            Ok(())
        }
        Err(_) => {
            driver.abort();
            if timeout(Duration::from_secs(2), &mut driver).await.is_err() {
                eprintln!("recovery driver join pending; Root owned-process cleanup required");
                std::process::exit(1);
            }
            Err(io::Error::other("recovery driver joined only after deadline").into())
        }
    }
}
async fn exercise() -> Result<(), Error> {
    let started = Instant::now();
    let configuration = configuration()?;
    let (mut inspector, connection) =
        timeout(Duration::from_secs(2), configuration.connect(NoTls)).await??;
    let inspector_driver = tokio::spawn(connection);
    let initial = journey::initial().map_err(repository_error)?;
    let codec = NativeCodecLimits {
        maximum_document_bytes: 1024 * 1024,
        maximum_allocated_bytes: 2 * 1024 * 1024,
        maximum_collection_items: 1024,
        maximum_text_bytes: 4096,
    };
    let fence = [0x83; 16];
    local_demo_scope::initialize(
        &mut inspector,
        &initial,
        fence,
        [0x81; 32],
        [0x82; 32],
        codec,
    )
    .await
    .map_err(|_| io::Error::other("registered local recovery database initialization failed"))?;
    let mut direct = configuration.clone();
    direct.application_name("df-engine-recovery-repository-01");
    let mut grants = configuration.clone();
    grants.application_name("df-engine-recovery-grant-01");
    let (grant_client, grant_connection) =
        timeout(Duration::from_secs(2), grants.connect(NoTls)).await??;
    let grant_driver = tokio::spawn(grant_connection);
    let issuer = LocalDemoScopeIssuer::new(
        tokio::runtime::Handle::current(),
        grant_client,
        initial.basis().session,
        fence,
    )
    .map_err(repository_error)?;
    let mut calls = Calls {
        count: 0,
        deadline: started + Duration::from_secs(45),
    };
    let prep_counter = Arc::new(AtomicUsize::new(0));
    let mut prep = RunningActor::start(
        repository(&direct, &direct, codec).await?,
        issuer,
        initial,
        codec,
        prep_counter.clone(),
    )?;
    let preparation = async {
        let first = join(&prep.service, 0x91, &mut calls).await?;
        let second = join(&prep.service, 0x92, &mut calls).await?;
        Ok::<_, Error>((first, second))
    }
    .await;
    let prepared = prep.close().await?;
    let (first, second) = preparation?;
    required(prep_counter.load(Ordering::SeqCst) == 2)?;
    let baseline = prepared.checkpoint;
    let listener = timeout(
        Duration::from_secs(2),
        tokio::net::TcpListener::bind(("127.0.0.1", 55518)),
    )
    .await??;
    let mut proxy = tokio::spawn(async move {
        let (frontend, _) = timeout(Duration::from_secs(5), listener.accept())
            .await
            .map_err(|_| local_demo_scope::ProxyError::Deadline)?
            .map_err(|_| local_demo_scope::ProxyError::Io)?;
        drop(listener);
        let postgres = timeout(
            Duration::from_secs(2),
            tokio::net::TcpStream::connect(("127.0.0.1", 55517)),
        )
        .await
        .map_err(|_| local_demo_scope::ProxyError::Deadline)?
        .map_err(|_| local_demo_scope::ProxyError::Io)?;
        local_demo_scope::drop_commit_acknowledgement(
            frontend,
            postgres,
            local_demo_scope::ProxyBounds {
                maximum_frame_bytes: 2 * 1024 * 1024,
                maximum_suppressed_response_bytes: 4096,
                deadline: Instant::now() + Duration::from_secs(30),
            },
        )
        .await
    });
    // Config::port appends, so build a fresh single-host proxy configuration.
    let proxied = trusted_configuration(55518, "df-engine-recovery-repository-01");
    let counter = Arc::new(AtomicUsize::new(0));
    let mut fault = RunningActor::start(
        repository(&proxied, &direct, codec).await?,
        prepared.issuer,
        baseline.clone(),
        codec,
        counter.clone(),
    )?;
    let original = create(&baseline, 0x93);
    let flow = async {
        expect_unknown_initial_submission(
            submit(&fault.service, &original, first, &mut calls).await,
        )?;
        required(counter.load(Ordering::SeqCst) == 1)?;
        required(*fault.service.updates.borrow() == baseline)?;
        let before = durable(&inspector, 0x93).await?;
        required(before.counts[3] == 0 && before.intents.is_empty())?;
        let mut different = create(&baseline, 0x94);
        required(
            submit(&fault.service, &different, second, &mut calls)
                .await
                .is_err_and(|status| status.code() == tonic::Code::Unavailable),
        )?;
        let mut conflict = original.clone();
        conflict
            .character
            .as_mut()
            .ok_or_else(|| io::Error::other("fixture creation absent"))?
            .name = "Different request".to_owned();
        required(
            submit(&fault.service, &conflict, first, &mut calls)
                .await
                .is_err_and(|status| status.code() == tonic::Code::Unavailable),
        )?;
        required(
            submit(&fault.service, &original, [0x82; 32], &mut calls)
                .await
                .is_err_and(|status| status.code() == tonic::Code::PermissionDenied),
        )?;
        inspector
            .execute(
                "UPDATE df_local_demo.grants SET active=false WHERE credential=$1::bytea",
                &[&first.as_slice()],
            )
            .await?;
        required(
            submit(&fault.service, &original, first, &mut calls)
                .await
                .is_err_and(|status| status.code() == tonic::Code::PermissionDenied),
        )?;
        inspector
            .execute(
                "UPDATE df_local_demo.grants SET active=true WHERE credential=$1::bytea",
                &[&first.as_slice()],
            )
            .await?;
        required(
            counter.load(Ordering::SeqCst) == 1 && durable(&inspector, 0x93).await? == before,
        )?;
        let recovered = committed(submit(&fault.service, &original, first, &mut calls).await?)?;
        required(
            recovered.replayed
                && matches!(
                    recovered.outcome,
                    Some(rpc::decision_receipt::Outcome::Accepted(_))
                ),
        )?;
        required(
            counter.load(Ordering::SeqCst) == 1 && durable(&inspector, 0x93).await? == before,
        )?;
        let restored = snapshot(&fault.service, &mut calls).await?;
        let restored_bytes =
            local_demo_scope::encode_owned_demo_checkpoint(&restored.checkpoint, codec)
                .map_err(repository_error)?;
        required(restored_bytes == before.envelope)?;
        let decision = restored
            .checkpoint
            .state()
            .decisions
            .iter()
            .find(|decision| decision.operation.as_bytes() == &[0x93; 16])
            .ok_or_else(|| io::Error::other("retained original decision absent"))?;
        let expected = wire::receipt(
            restored.checkpoint.basis(),
            decision.operation,
            rpc::decision_receipt::Outcome::Accepted(Box::new(
                journey::accepted(decision).map_err(repository_error)?,
            )),
            true,
        );
        required(recovered == expected && !before.receipt.is_empty())?;

        required(
            restored.checkpoint.basis().revision
                == baseline
                    .basis()
                    .revision
                    .next_sequence()
                    .map_err(|_| io::Error::other("fixture revision exhausted"))?,
        )?;
        let retry = committed(submit(&fault.service, &original, first, &mut calls).await?)?;
        required(
            retry.replayed
                && retry.outcome == recovered.outcome
                && retry.revision == recovered.revision,
        )?;
        different = create(&restored.checkpoint, 0x94);
        let later = committed(submit(&fault.service, &different, second, &mut calls).await?)?;
        required(!later.replayed && counter.load(Ordering::SeqCst) == 2)?;
        let after = durable(&inspector, 0x94).await?;
        required(
            after.counts[0] == before.counts[0] + 1 && after.counts[2] == before.counts[2] + 1,
        )?;
        let duplicate = committed(submit(&fault.service, &different, second, &mut calls).await?)?;
        required(
            duplicate.replayed
                && duplicate.outcome == later.outcome
                && durable(&inspector, 0x94).await? == after,
        )?;
        let current = snapshot(&fault.service, &mut calls).await?;
        required(
            local_demo_scope::encode_owned_demo_checkpoint(&current.checkpoint, codec)
                .map_err(repository_error)?
                == after.envelope,
        )?;
        // A real second authorized owner advances the durable head. The original
        // actor's known CAS failure must retain a reload route distinct from Unknown.
        let (external_client, external_connection) =
            timeout(Duration::from_secs(2), grants.connect(NoTls)).await??;
        let external_driver = tokio::spawn(external_connection);
        let external_issuer = LocalDemoScopeIssuer::new(
            tokio::runtime::Handle::current(),
            external_client,
            current.checkpoint.basis().session,
            fence,
        )
        .map_err(repository_error)?;
        let mut external = RunningActor::start(
            repository(&direct, &direct, codec).await?,
            external_issuer,
            current.checkpoint.clone(),
            codec,
            Arc::new(AtomicUsize::new(0)),
        )?;
        let advance = async {
            let begin = action(
                &current.checkpoint,
                0x95,
                rpc::GameplayActionKind::BeginStory,
            );
            required(
                !committed(submit(&external.service, &begin, first, &mut calls).await?)?.replayed,
            )?;
            snapshot(&external.service, &mut calls)
                .await
                .map(|snapshot| snapshot.checkpoint)
        }
        .await;
        let external_exit = external.close().await?;
        drop(external_exit.issuer);
        join_driver(external_driver).await?;
        let advanced = advance?;
        let stale = action(
            &current.checkpoint,
            0x96,
            rpc::GameplayActionKind::BeginStory,
        );
        required(
            submit(&fault.service, &stale, first, &mut calls)
                .await
                .is_err_and(|status| status.code() == tonic::Code::Unavailable),
        )?;
        let known = snapshot(&fault.service, &mut calls).await?;
        required(known.fenced && !known.uncertain && known.checkpoint == current.checkpoint)?;
        let courier = action(&advanced, 0x97, rpc::GameplayActionKind::AskCourier);
        required(!committed(submit(&fault.service, &courier, first, &mut calls).await?)?.replayed)?;
        let resumed = snapshot(&fault.service, &mut calls).await?;
        required(!resumed.fenced && !resumed.uncertain)?;
        required(
            local_demo_scope::encode_owned_demo_checkpoint(&resumed.checkpoint, codec)
                .map_err(repository_error)?
                == durable(&inspector, 0x97).await?.envelope,
        )?;
        required(counter.load(Ordering::SeqCst) == 4)?;
        Ok::<_, Error>(resumed.checkpoint)
    }
    .await;
    let fault_exit = fault.close().await?;
    let proxy_join = timeout(Duration::from_secs(2), &mut proxy).await;
    let observation = match proxy_join {
        Ok(joined) => joined?.map_err(|error| {
            // Enum-only diagnostics never contain protocol bytes or credentials.
            eprintln!("owned ACK proxy {error:?}");
            io::Error::other(format!("owned ACK proxy {error:?}"))
        }),
        Err(_) => {
            proxy.abort();
            if timeout(Duration::from_secs(2), &mut proxy).await.is_err() {
                eprintln!("recovery proxy join pending; Root owned-process cleanup required");
                std::process::exit(1);
            }
            Err(io::Error::other("owned ACK proxy joined after deadline"))
        }
    };
    drop(fault_exit.issuer);
    join_driver(grant_driver).await?;
    // The primary Service/Actor assertion is retained even if its cleanup closes
    // the proxy before COMMIT. A proxy failure never certifies a committed write.
    let final_checkpoint = flow?;
    let observation = observation?;
    required(fault_exit.checkpoint == final_checkpoint)?;
    required(
        observation.frontend_commit_forwarded
            && observation.postgres_commit_complete_observed
            && observation.postgres_ready_idle_observed
            && observation.suppressed_response_bytes > 0,
    )?;
    let remaining = timeout(Duration::from_secs(2), async {
        loop {
            let count: i64 = inspector.query_one("SELECT count(*) FROM pg_stat_activity WHERE application_name LIKE 'df-engine-recovery-%' AND application_name <> 'df-engine-recovery-inspector-01'", &[]).await?.try_get(0)?;
            if count == 0 { return Ok::<_, tokio_postgres::Error>(count); }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }).await??;
    drop(inspector);
    join_driver(inspector_driver).await?;
    required(remaining == 0 && calls.count <= 20 && started.elapsed() <= Duration::from_secs(45))?;
    let calls = calls.count;
    std::fs::write(
        REPORT,
        format!(
            "{{\"pass\":true,\"actor_calls\":{calls},\"actor_calls_limit\":20,\"exercise_milliseconds\":{},\"exercise_milliseconds_limit\":45000,\"original_engine_reductions\":1,\"total_fault_engine_reductions\":4,\"exact_retry_stored_receipt\":true,\"physical_operation_rows_unchanged\":true,\"physical_fact_rows_unchanged\":true,\"physical_intent_rows_unchanged\":true,\"observed_intent_rows\":0,\"checkpoint_reload_byte_equality\":true,\"later_action_once\":true,\"known_reload_resumed\":true,\"proxy_commit_complete\":true,\"proxy_ready_idle\":true,\"proxy_suppressed_bytes\":{},\"proxy_task_joined\":true,\"actor_threads_joined\":3,\"native_repository_drivers_joined\":4,\"auxiliary_grant_drivers_joined\":2,\"inspector_driver_joined\":true,\"remaining_owned_backends\":{remaining},\"owned_backend_application_prefix\":\"df-engine-recovery-\"}}\n",
            started.elapsed().as_millis(),
            observation.suppressed_response_bytes
        ),
    )?;
    Ok(())
}
fn repository_error(error: df_session::submission::RepositoryError) -> Error {
    io::Error::other(format!("recovery repository {error:?}")).into()
}
fn expect_unknown_initial_submission(
    outcome: Result<rpc::SubmitActionResponse, tonic::Status>,
) -> Result<(), Error> {
    match outcome {
        Err(status) if status.code() == tonic::Code::Unavailable => Ok(()),
        Err(status) => Err(io::Error::other(format!(
            "initial submit expected unavailable; actual status {:?}",
            status.code()
        ))
        .into()),
        Ok(response) => {
            let classification = match response.outcome {
                Some(rpc::submit_action_response::Outcome::CommittedDecision(receipt)) => {
                    match receipt.outcome {
                        Some(rpc::decision_receipt::Outcome::Accepted(_)) => {
                            "committed accepted".to_owned()
                        }
                        Some(rpc::decision_receipt::Outcome::Rejected(rejection)) => format!(
                            "committed rejection {:?}",
                            rpc::RejectionCode::try_from(rejection.code)
                        ),
                        None => "committed outcome absent".to_owned(),
                    }
                }
                Some(rpc::submit_action_response::Outcome::OperationObservation(observation)) => {
                    format!(
                        "operation observation {:?}",
                        rpc::RejectionCode::try_from(observation.code)
                    )
                }
                None => "outcome absent".to_owned(),
            };
            Err(io::Error::other(format!(
                "initial submit expected unavailable; actual RPC success {classification}"
            ))
            .into())
        }
    }
}

/// Operator-only connecting proof using the existing registered recovery database.
/// Run separately on its fresh database; this does not reset or create a database.
#[test]
#[ignore = "requires ROOT registered fresh loopback recovery database and original resource guard"]
fn native_courier_committed_pending_actor_restart() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(courier_pending_actor_restart()).unwrap();
}

async fn courier_pending_actor_restart() -> Result<(), Error> {
    let started = Instant::now();
    let configuration = configuration()?;
    let (mut inspector, inspector_connection) =
        timeout(Duration::from_secs(2), configuration.connect(NoTls)).await??;
    let inspector_driver = tokio::spawn(inspector_connection);
    let (grant_client, grant_connection) =
        timeout(Duration::from_secs(2), configuration.connect(NoTls)).await??;
    let grant_driver = tokio::spawn(grant_connection);
    let codec = NativeCodecLimits {
        maximum_document_bytes: 1024 * 1024,
        maximum_allocated_bytes: 2 * 1024 * 1024,
        maximum_collection_items: 1024,
        maximum_text_bytes: 4096,
    };
    let initial = journey::initial().map_err(repository_error)?;
    let session = initial.basis().session;
    let fence = [0x83; 16];
    let initialized = local_demo_scope::initialize(
        &mut inspector,
        &initial,
        fence,
        [0x81; 32],
        [0x82; 32],
        codec,
    )
    .await;
    if initialized.is_err() {
        drop(grant_client);
        drop(inspector);
        timeout(Duration::from_secs(2), grant_driver).await???;
        timeout(Duration::from_secs(2), inspector_driver).await???;
        return Err(io::Error::other("fresh registered courier database required").into());
    }
    let issuer = LocalDemoScopeIssuer::new(
        tokio::runtime::Handle::current(),
        grant_client,
        initial.basis().session,
        fence,
    )
    .map_err(repository_error)?;
    let reductions = Arc::new(AtomicUsize::new(0));
    let mut actor = RunningActor::start(
        repository(&configuration, &configuration, codec).await?,
        issuer,
        initial,
        codec,
        reductions.clone(),
    )?;
    let mut calls = Calls {
        count: 0,
        deadline: started + Duration::from_secs(45),
    };
    let preparation = async {
        let first = join(&actor.service, 0x91, &mut calls).await?;
        let second = join(&actor.service, 0x92, &mut calls).await?;
        let current = actor.service.updates.borrow().clone();
        committed(submit(&actor.service, &create(&current, 0x93), first, &mut calls).await?)?;
        let current = actor.service.updates.borrow().clone();
        committed(submit(&actor.service, &create(&current, 0x94), second, &mut calls).await?)?;
        let current = actor.service.updates.borrow().clone();
        committed(
            submit(
                &actor.service,
                &action(&current, 0x95, rpc::GameplayActionKind::BeginStory),
                first,
                &mut calls,
            )
            .await?,
        )?;
        Ok::<_, Error>(first)
    }
    .await;
    let prepared = actor.close().await?;
    let result = async {
    if let Ok(first) = preparation {
        let body = action(&prepared.checkpoint, 0x96, rpc::GameplayActionKind::AskCourier);
        let principal = journey::participants(&prepared.checkpoint).first().ok_or_else(|| io::Error::other("courier recipient absent"))?.member;
        let input = wire::journey_input(&body, principal, &prepared.checkpoint)?;
        let native_repository = repository(&configuration, &configuration, codec).await?;
        let count = reductions.clone();
        let commit_thread = std::thread::Builder::new().name("df-courier-source-commit-owner".to_owned()).spawn(move || {
            use df_session::inbox::{AdmissionSequence, Reducer};
            use df_session::submission::{OwnedInput, SubmissionOutcome};
            let mut issuer = prepared.issuer;
            let scope = issuer.issue(&first, model::random().map_err(repository_error)?, Sha256::digest(body.encode_to_vec()).into(), input.clone(), model::pins().map_err(repository_error)?).map_err(repository_error)?;
            let (updates, _) = watch::channel(prepared.checkpoint.clone());
            let (publication, notifications) = actor::Publication::new(updates);
            let mut owner = DurableOwner::new(native_repository, actor::Engine(Some(count)), publication, prepared.checkpoint, 4096).map_err(repository_error)?;
            let (item, receipt) = OwnedInput::new(df_observe::OperationContext { trace_parent: String::new(), build: crate::BUILD_ID.to_owned() }, scope, input);
            owner.reduce(AdmissionSequence(0), item);
            let confirmed = matches!(receipt.try_recv(), Ok(SubmissionOutcome::Confirmed(_)));
            let woken = notifications.try_recv().is_ok();
            let checkpoint = owner.checkpoint().clone();
            let mut repository = owner.into_repository();
            repository.close().map_err(repository_error)?;
            required(confirmed && woken)?;
            Ok::<_, Error>(ActorExit { checkpoint, issuer })
        })?;
        let pending = tokio::task::spawn_blocking(move || commit_thread.join()).await?.map_err(|_| io::Error::other("courier source commit owner panicked"))??;
        required(reductions.load(Ordering::SeqCst) == 6)?;
        let effect = pending.checkpoint.state().intents.first().ok_or_else(|| io::Error::other("durable courier effect absent"))?;
        required(effect.status == df_model::checkpoint::DurableStatus::Pending)?;
        required(super::courier_ai::saved_response(&pending.checkpoint, principal).map_err(repository_error)?.is_none())?;
        let physical_pending = durable(&inspector, 0x96).await?;
        required(physical_pending.counts[3] == 1 && !physical_pending.intents.is_empty())?;
        let mut restarted = RunningActor::start(repository(&configuration, &configuration, codec).await?, pending.issuer, pending.checkpoint, codec, reductions.clone())?;
        let resumed = snapshot(&restarted.service, &mut calls).await;
        let resumed_exit = restarted.close().await?;
        let resumed = resumed?;
        required(reductions.load(Ordering::SeqCst) == 7)?;
        required(super::courier_ai::saved_response(&resumed.checkpoint, principal).map_err(repository_error)? == Some(super::courier_ai::RESPONSE))?;
        let accepted = durable(&inspector, 0x96).await?;
        required(accepted.counts[0] == physical_pending.counts[0] + 1 && accepted.intents == physical_pending.intents)?;
        let mut again = RunningActor::start(repository(&configuration, &configuration, codec).await?, resumed_exit.issuer, resumed_exit.checkpoint, codec, reductions.clone())?;
        let unchanged = snapshot(&again.service, &mut calls).await;
        let final_exit = again.close().await?;
        drop(final_exit.issuer);
        let unchanged = unchanged?;
        required(unchanged.checkpoint == resumed.checkpoint && reductions.load(Ordering::SeqCst) == 7)?;
        required(durable(&inspector, 0x96).await? == accepted)?;
        required(Instant::now() < calls.deadline)?;
        std::fs::write(COURIER_REPORT, format!("{{\"pass\":true,\"actor_calls\":{},\"actor_calls_limit\":20,\"engine_reductions\":7,\"source_commit_then_real_wake\":true,\"pending_durable_intent_on_actor_restart\":true,\"native_startup_completed_once\":true,\"second_actor_restart_unchanged\":true,\"immutable_declaration_unchanged\":true,\"process_restart_verified\":false}}\n", calls.count))?;
        Ok(())
    } else {
        let issuer = prepared.issuer;
        drop(issuer);
        Err(io::Error::other("courier source preparation refused").into())
    }
    }.await;
    let released = local_demo_scope::release_owner(&inspector, session, fence).await;
    drop(inspector);
    timeout(Duration::from_secs(2), grant_driver).await???;
    timeout(Duration::from_secs(2), inspector_driver).await???;
    released.map_err(repository_error)?;
    result
}
#[test]
fn initial_submission_diagnostics_retain_status_without_private_payload() {
    assert!(expect_unknown_initial_submission(Err(tonic::Status::unavailable("private"))).is_ok());
    let denied =
        expect_unknown_initial_submission(Err(tonic::Status::permission_denied("private")))
            .unwrap_err()
            .to_string();
    assert_eq!(
        denied,
        "initial submit expected unavailable; actual status PermissionDenied"
    );
    assert!(!denied.contains("private"));
    assert!(
        expect_unknown_initial_submission(Ok(rpc::SubmitActionResponse::default()))
            .unwrap_err()
            .to_string()
            .contains("RPC success")
    );
}
#[test]
fn recovery_creation_body_passes_actual_source_character_validator() {
    let initial = journey::initial().unwrap();
    let body = create(&initial, 0x93);
    let character = body.character.unwrap();
    let selections = character
        .choices
        .iter()
        .map(|choice| (choice.group_id.as_str(), choice.option_id.as_str()))
        .collect::<Vec<_>>();
    assert!(df_rules::local_journey::validate_character(&character.name, &selections).is_ok());
    assert!(matches!(
        df_rules::local_journey::validate_character("Recovery Fighter", &selections),
        Err(df_rules::local_journey::JourneyRuleError::InvalidName)
    ));
}
#[test]
fn trusted_direct_and_proxy_configurations_each_have_one_host_and_one_port() {
    for port in [55517, 55518] {
        let configuration = trusted_configuration(port, "df-engine-recovery-config-test");
        assert_eq!(configuration.get_ports(), &[port]);
        assert!(
            matches!(configuration.get_hosts(), [tokio_postgres::config::Host::Tcp(host)] if host == "127.0.0.1")
        );
        assert_eq!(
            configuration.get_dbname(),
            Some("df_gameplay_demo_20261005_recovery05")
        );
        assert_eq!(
            configuration.get_user(),
            Some("df_gameplay_demo_admin_20261004")
        );
    }
}
#[test]
#[ignore = "Root registered recovery05 PG55517/proxy55518 and explicit finite release required"]
fn actual_registered_pg_generated_service_recovers_lost_commit_ack() {
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("fixture runtime");
    runtime
        .block_on(exercise())
        .expect("actual generated-service recovery proof");
}
