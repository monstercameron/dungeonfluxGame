//! Thin generated-RPC presentation for the finite local gameplay slice.
//! No die generation, total calculation, outcome selection, or clue authority lives here.
use df_protocol::common as rpc;
use df_rpc_bridge::{BrowserChannel, BrowserConnection};
use futures::future::{AbortHandle, Abortable};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{Document, Element};

type ActionClient = rpc::action_service_client::ActionServiceClient<BrowserChannel>;
type SessionClient = rpc::session_service_client::SessionServiceClient<BrowserChannel>;
type Callback = (Element, Closure<dyn FnMut(web_sys::Event)>);
struct Client {
    document: Document,
    root: Element,
    role: &'static str,
    credential: String,
    generation: u64,
    revision: Option<rpc::SessionRevision>,
    last_request: Option<rpc::SubmitActionRequest>,
    action: Option<ActionClient>,
    connection: Option<BrowserConnection>,
    watch_abort: Option<AbortHandle>,
    callbacks: Vec<Callback>,
    busy: bool,
}
thread_local! { static CLIENTS:RefCell<Vec<Rc<RefCell<Client>>>>=const { RefCell::new(Vec::new()) }; }
fn element(
    document: &Document,
    parent: &Element,
    tag: &str,
    class: &str,
    text: &str,
) -> Result<Element, JsValue> {
    let node = document.create_element(tag)?;
    node.set_class_name(class);
    node.set_text_content(Some(text));
    parent.append_child(&node)?;
    Ok(node)
}
fn request<T>(body: T, credential: &str) -> Result<tonic::Request<T>, String> {
    let mut request = tonic::Request::new(body);
    request.set_timeout(std::time::Duration::from_secs(5));
    request.metadata_mut().insert(
        "x-df-local-binding",
        credential
            .parse()
            .map_err(|_| "local binding unavailable")?,
    );
    Ok(request)
}
fn feedback(client: &Client, text: &str) {
    if let Ok(Some(node)) = client.root.query_selector(".feedback") {
        node.set_text_content(Some(text));
    }
}
fn status(client: &Client, text: &str) {
    if let Ok(Some(node)) = client.root.query_selector(".connection") {
        node.set_text_content(Some(text));
    }
}
fn retire_callbacks(client: &mut Client) {
    for (node, callback) in std::mem::take(&mut client.callbacks) {
        if node
            .remove_event_listener_with_callback("click", callback.as_ref().unchecked_ref())
            .is_err()
        {
            status(client, "A retired input listener could not be removed.");
        }
    }
}
fn button(
    client: &Rc<RefCell<Client>>,
    parent: &Element,
    label: &str,
    kind: &'static str,
) -> Result<(), JsValue> {
    let document = client.borrow().document.clone();
    let node = element(&document, parent, "button", "", label)?;
    let owned = client.clone();
    let callback = Closure::wrap(Box::new(move |_: web_sys::Event| match kind {
        "reconnect" => connect(owned.clone()),
        "submit" => submit(owned.clone(), false),
        "replay" => submit(owned.clone(), true),
        _ => {}
    }) as Box<dyn FnMut(web_sys::Event)>);
    node.add_event_listener_with_callback("click", callback.as_ref().unchecked_ref())?;
    client.borrow_mut().callbacks.push((node, callback));
    Ok(())
}
fn render(client: &Rc<RefCell<Client>>, view: rpc::ViewMessage) -> Result<(), JsValue> {
    {
        let state = client.borrow();
        if let (Some(previous), Some(next)) = (state.revision.as_ref(), view.revision.as_ref())
            && previous.epoch == next.epoch
            && next.sequence < previous.sequence
        {
            return Ok(());
        }
    }
    let (document, root, role) = {
        let mut state = client.borrow_mut();
        retire_callbacks(&mut state);
        state.revision = view.revision;
        (state.document.clone(), state.root.clone(), state.role)
    };
    let surface = root
        .query_selector(".view")?
        .ok_or_else(|| JsValue::from_str("mounted gameplay view unavailable"))?;
    surface.set_text_content(None);
    let art = element(&document, &surface, "img", "art", "")?;
    art.set_attribute("src", "/assets/ui/scenes/mara-harbor-v4.png")?;
    art.set_attribute(
        "alt",
        "Mara at the harbor among lanterns, sailing ships, and the old loading pier",
    )?;
    let body = element(&document, &surface, "div", "scene-copy", "")?;
    match view
        .audience
        .ok_or_else(|| JsValue::from_str("server audience absent"))?
    {
        rpc::view_message::Audience::Player(view) if role == "player" => {
            element(
                &document,
                &body,
                "div",
                "eyebrow",
                "Mara · your investigation",
            )?;
            element(&document, &body, "h2", "", "The Broken Seal")?;
            element(&document, &body, "p", "narration", &view.narration)?;
            if view.action_available {
                let action = element(&document, &body, "div", "", "")?;
                button(client, &action, "Examine the harbor seal", "submit")?;
                if let Some(node) = action.first_element_child() {
                    node.set_class_name("offered");
                }
                element(
                    &document,
                    &action,
                    "p",
                    "scope",
                    "Intelligence (Investigation) · DC 15\nMara: Intelligence +2, Investigation proficiency +2. One attempt; failure loses the clue.",
                )?;
                if client.borrow().last_request.is_none() {
                    let observed_revision = client.borrow().revision;
                    client.borrow_mut().last_request = Some(rpc::SubmitActionRequest {
                        session_id: Some(rpc::SessionId {
                            value: Some(vec![0x41; 16]),
                        }),
                        run_id: Some(rpc::RunId {
                            value: Some(vec![0x42; 16]),
                        }),
                        observed_revision,
                        operation_id: None,
                        offer_id: view.offer_id,
                        action_kind: rpc::GameplayActionKind::ExamineHarborSeal as i32,
                    });
                }
            }
            if let Some(check) = view.check {
                let panel = element(&document, &body, "div", "result", "")?;
                element(
                    &document,
                    &panel,
                    "div",
                    "check-label",
                    "SERVER-RESOLVED · INTELLIGENCE (INVESTIGATION)",
                )?;
                element(
                    &document,
                    &panel,
                    "div",
                    "dice",
                    &format!(
                        "{} + {} + {} = {}",
                        check.die, check.ability_modifier, check.proficiency_bonus, check.total
                    ),
                )?;
                element(
                    &document,
                    &panel,
                    "p",
                    "",
                    &format!(
                        "{} · DC {}",
                        if check.succeeded {
                            "Success"
                        } else {
                            "Failure"
                        },
                        check.difficulty_class
                    ),
                )?;
                element(
                    &document,
                    &panel,
                    "p",
                    "receipt",
                    "Saved to your game. Reconnecting preserves this decision.",
                )?;
                if !view.private_clue.is_empty() {
                    element(&document, &body, "div", "eyebrow", "Private discovery")?;
                    element(&document, &body, "p", "clue", &view.private_clue)?;
                }
            }
            element(&document, &body, "p", "feedback", "")?;
            let tools = element(&document, &body, "div", "tools", "")?;
            button(client, &tools, "Confirm saved decision", "replay")?;
            if let Some(node) = tools.last_element_child() {
                node.set_class_name("confirm-decision");
                if client
                    .borrow()
                    .last_request
                    .as_ref()
                    .is_none_or(|request| request.operation_id.is_none())
                {
                    node.set_attribute("disabled", "")?;
                }
            }
            button(client, &tools, "Reconnect", "reconnect")?;
        }
        rpc::view_message::Audience::Display(view) if role == "display" => {
            if view.scene_asset != "assets/ui/scenes/mara-harbor-v4.png" {
                return Err(JsValue::from_str("unadmitted scene asset"));
            }
            element(
                &document,
                &body,
                "div",
                "eyebrow",
                "Shared adventure · harbor",
            )?;
            element(&document, &body, "h1", "", "The Broken Seal")?;
            element(&document, &body, "p", "narration", &view.narration)?;
            let tools = element(&document, &body, "div", "tools", "")?;
            button(client, &tools, "Reconnect display", "reconnect")?;
        }
        _ => {
            return Err(JsValue::from_str(
                "server view audience mismatches this binding",
            ));
        }
    }
    let sequence = client
        .borrow()
        .revision
        .as_ref()
        .and_then(|revision| revision.sequence)
        .unwrap_or(0);
    root.set_attribute("data-server-sequence", &sequence.to_string())?;
    status(
        &client.borrow(),
        &format!("Connected · saved game revision {sequence}"),
    );
    Ok(())
}
fn connect(client: Rc<RefCell<Client>>) {
    let (generation, credential, role) = {
        let mut state = client.borrow_mut();
        if let Some(abort) = state.watch_abort.take() {
            abort.abort();
        }
        if let Some(connection) = state.connection.take() {
            connection.close();
        }
        state.action.take();
        state.busy = false;
        state.generation += 1;
        status(&state, "Reconnecting to your saved game…");
        (state.generation, state.credential.clone(), state.role)
    };
    let (abort, registration) = AbortHandle::new_pair();
    client.borrow_mut().watch_abort = Some(abort);
    let observed = client.borrow().revision;
    let owned = client.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let run = async {
            let url = crate::browser::tunnel_url("/gameplay/tunnel")?;
            let (connection, channel) = BrowserConnection::connect(&url)
                .await
                .map_err(|_| "Connection unavailable")?;
            if owned.borrow().generation != generation {
                connection.close();
                return Ok::<(), String>(());
            }
            let mut sessions = SessionClient::new(channel.clone())
                .max_decoding_message_size(8192)
                .max_encoding_message_size(8192);
            owned.borrow_mut().action = Some(
                ActionClient::new(channel)
                    .max_decoding_message_size(8192)
                    .max_encoding_message_size(8192),
            );
            owned.borrow_mut().connection = Some(connection);
            let view_request = rpc::WatchViewRequest {
                session_id: Some(rpc::SessionId {
                    value: Some(vec![0x41; 16]),
                }),
                run_id: Some(rpc::RunId {
                    value: Some(vec![0x42; 16]),
                }),
                client_binding_id: Some(rpc::ClientBindingId {
                    value: Some(vec![if role == "player" { 0x71 } else { 0x72 }; 16]),
                }),
                after_revision: observed,
            };
            let mut watch_request = tonic::Request::new(view_request);
            watch_request.metadata_mut().insert(
                "x-df-local-binding",
                credential
                    .parse()
                    .map_err(|_| "Local binding unavailable")?,
            );
            let mut stream = sessions
                .watch(watch_request)
                .await
                .map_err(|_| "Saved game view unavailable")?
                .into_inner();
            while let Some(view) = stream
                .message()
                .await
                .map_err(|_| "Connection interrupted; reconnect to recover")?
            {
                if owned.borrow().generation != generation {
                    return Ok(());
                }
                render(&owned, view).map_err(|_| "Server view could not be displayed")?;
            }
            Ok(())
        };
        let outcome = Abortable::new(run, registration).await;
        if owned.borrow().generation == generation {
            if let Some(connection) = owned.borrow_mut().connection.take() {
                connection.close();
            }
            owned.borrow_mut().action.take();
            let message = match outcome {
                Ok(Ok(())) => "Game connection closed.",
                Ok(Err(ref error)) => error.as_str(),
                Err(_) => "Connection cancelled.",
            };
            status(&owned.borrow(), message);
        }
    });
}
fn operation() -> Result<rpc::OperationId, String> {
    let window = web_sys::window().ok_or("Browser unavailable")?;
    let crypto = window.crypto().map_err(|_| "Input identity unavailable")?;
    let mut bytes = [0; 16];
    crypto
        .get_random_values_with_u8_array(&mut bytes)
        .map_err(|_| "Input identity unavailable")?;
    if bytes == [0; 16] {
        return Err("Input identity unavailable".to_owned());
    }
    Ok(rpc::OperationId {
        value: Some(bytes.to_vec()),
    })
}
fn submit(client: Rc<RefCell<Client>>, replay: bool) {
    let prepared = (|| -> Result<_, String> {
        let mut state = client.borrow_mut();
        if state.busy {
            return Err("Waiting for the server decision…".to_owned());
        }
        let action = state
            .action
            .clone()
            .ok_or("Reconnect before sending your input")?;
        let mut body = state
            .last_request
            .clone()
            .ok_or("No current offered action")?;
        if !replay && body.operation_id.is_none() {
            body.operation_id = Some(operation()?);
            state.last_request = Some(body.clone());
        }
        if body.operation_id.is_none() {
            return Err("No saved operation to confirm".to_owned());
        }
        state.busy = true;
        feedback(&state, "Waiting for the game server…");
        if let Ok(Some(button)) = state.root.query_selector(".confirm-decision") {
            button
                .remove_attribute("disabled")
                .map_err(|_| "Decision confirmation control unavailable")?;
        }
        Ok((action, body, state.credential.clone(), state.generation))
    })();
    let (mut action, body, credential, generation) = match prepared {
        Ok(value) => value,
        Err(error) => {
            feedback(&client.borrow(), &error);
            return;
        }
    };
    wasm_bindgen_futures::spawn_local(async move {
        let submitted = body.clone();
        let outcome = match request(body, &credential) {
            Ok(request) => action
                .submit(request)
                .await
                .map(|reply| reply.into_inner())
                .map_err(|_| "Decision not received. Confirm the same operation or reconnect."),
            Err(_) => Err("Input binding unavailable"),
        };
        if client.borrow().generation != generation {
            return;
        }
        let mut state = client.borrow_mut();
        state.busy = false;
        match outcome {
            Ok(response) => {
                let receipt = match response.outcome {
                    Some(rpc::submit_action_response::Outcome::CommittedDecision(receipt)) => {
                        receipt
                    }
                    Some(rpc::submit_action_response::Outcome::OperationObservation(
                        observation,
                    )) => {
                        feedback(
                            &state,
                            match rpc::RejectionCode::try_from(observation.code) {
                                Ok(rpc::RejectionCode::OperationConflict) => {
                                    "That operation belongs to different input. Reconnect to recover."
                                }
                                Ok(rpc::RejectionCode::OperationExpired) => {
                                    "That operation has expired. Recover the saved game before sending new input."
                                }
                                _ => {
                                    "The game authority requires recovery. Reconnect before continuing."
                                }
                            },
                        );
                        return;
                    }
                    None => {
                        feedback(&state, "The server returned no operation outcome.");
                        return;
                    }
                };
                if receipt.session_id != submitted.session_id
                    || receipt.run_id != submitted.run_id
                    || receipt.operation_id != submitted.operation_id
                    || receipt.revision.is_none()
                {
                    feedback(
                        &state,
                        "The decision belongs to a different scope. Reconnect to recover.",
                    );
                    return;
                }
                match receipt.outcome {
                    Some(rpc::decision_receipt::Outcome::Accepted(_)) => feedback(
                        &state,
                        if receipt.replayed {
                            "Original saved decision confirmed. The die was not rolled again."
                        } else {
                            "Your action is committed to the game."
                        },
                    ),
                    Some(rpc::decision_receipt::Outcome::Rejected(rejection)) => feedback(
                        &state,
                        match rpc::RejectionCode::try_from(rejection.code) {
                            Ok(rpc::RejectionCode::StaleOffer) => {
                                "That investigation is no longer available."
                            }
                            Ok(rpc::RejectionCode::OperationConflict) => {
                                "That operation belongs to different input. Reconnect to recover."
                            }
                            _ => "The server declined this input. Reconnect to recover.",
                        },
                    ),
                    None => feedback(&state, "The server returned no decision outcome."),
                }
                let sequence = receipt
                    .revision
                    .and_then(|revision| revision.sequence)
                    .unwrap_or(0);
                if state
                    .root
                    .set_attribute("data-last-receipt-sequence", &sequence.to_string())
                    .is_err()
                {
                    status(&state, "Receipt display metadata unavailable.");
                }
                if state
                    .root
                    .set_attribute(
                        "data-last-receipt-replayed",
                        if receipt.replayed { "true" } else { "false" },
                    )
                    .is_err()
                {
                    status(&state, "Receipt display metadata unavailable.");
                }
            }
            Err(error) => feedback(&state, error),
        }
    });
}
pub(super) fn start() -> Result<(), JsValue> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| JsValue::from_str("document unavailable"))?;
    for role in ["player", "display"] {
        let Some(root) = document.get_element_by_id(role) else {
            continue;
        };
        let credential = root
            .get_attribute("data-local-binding")
            .ok_or_else(|| JsValue::from_str("local binding absent"))?;
        root.remove_attribute("data-local-binding")?;
        let client = Rc::new(RefCell::new(Client {
            document: document.clone(),
            root,
            role,
            credential,
            generation: 0,
            revision: None,
            last_request: None,
            action: None,
            connection: None,
            watch_abort: None,
            callbacks: vec![],
            busy: false,
        }));
        CLIENTS.with(|clients| clients.borrow_mut().push(client.clone()));
        connect(client);
    }
    Ok(())
}
