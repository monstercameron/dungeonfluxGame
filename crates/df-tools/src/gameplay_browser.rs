//! Thin server-projected room, creation, dialogue and combat clients.
use df_protocol::common as rpc;
use df_rpc_bridge::{BrowserChannel, BrowserConnection};
use df_ui::{
    CharacterAction, CharacterActionKind, CharacterFact, CharacterGroup, CharacterLimits,
    CharacterOption, CharacterPhaseSurface, CharacterPhaseView, CharacterStatus,
};
use futures::future::{AbortHandle, Abortable};
use std::{cell::RefCell, rc::Rc};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{Document, Element, HtmlSelectElement};

type ActionClient = rpc::action_service_client::ActionServiceClient<BrowserChannel>;
type SessionClient = rpc::session_service_client::SessionServiceClient<BrowserChannel>;
type RoomClient = rpc::room_service_client::RoomServiceClient<BrowserChannel>;
type Callback = (Element, Closure<dyn FnMut(web_sys::Event)>);
struct Client {
    document: Document,
    root: Element,
    role: &'static str,
    credential: String,
    room_code: String,
    session: Option<rpc::SessionId>,
    run: Option<rpc::RunId>,
    binding: Option<rpc::ClientBindingId>,
    generation: u64,
    revision: Option<rpc::SessionRevision>,
    last_view: Option<rpc::ViewMessage>,
    join_request: Option<rpc::JoinRoomRequest>,
    last_request: Option<rpc::SubmitActionRequest>,
    first_attack: Option<rpc::SubmitActionRequest>,
    last_confirmed: bool,
    action: Option<ActionClient>,
    connection: Option<BrowserConnection>,
    watch_abort: Option<AbortHandle>,
    callbacks: Vec<Callback>,
    creation: Option<CharacterPhaseSurface>,
    busy: bool,
}
thread_local! {static CLIENTS:RefCell<Vec<Rc<RefCell<Client>>>>=const {RefCell::new(Vec::new())};}
fn node(
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
fn status(client: &Client, text: &str) {
    if let Ok(Some(node)) = client.root.query_selector(".connection") {
        node.set_text_content(Some(text));
    }
}
fn feedback(client: &Client, text: &str) {
    if let Ok(Some(node)) = client.root.query_selector(".feedback") {
        node.set_text_content(Some(text));
    }
}
fn clear_callbacks(client: &mut Client) {
    for (element, callback) in std::mem::take(&mut client.callbacks) {
        if element
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
    mut activate: impl FnMut(Rc<RefCell<Client>>) + 'static,
) -> Result<Element, JsValue> {
    let element = node(&client.borrow().document, parent, "button", "", label)?;
    let weak = Rc::downgrade(client);
    let callback = Closure::wrap(Box::new(move |_: web_sys::Event| {
        if let Some(client) = weak.upgrade() {
            activate(client);
        }
    }) as Box<dyn FnMut(web_sys::Event)>);
    element.add_event_listener_with_callback("click", callback.as_ref().unchecked_ref())?;
    client
        .borrow_mut()
        .callbacks
        .push((element.clone(), callback));
    Ok(element)
}
fn random<const N: usize>() -> Result<Vec<u8>, String> {
    let window = web_sys::window().ok_or("Browser unavailable")?;
    let crypto = window.crypto().map_err(|_| "Input identity unavailable")?;
    let mut bytes = [0; N];
    crypto
        .get_random_values_with_u8_array(&mut bytes)
        .map_err(|_| "Input identity unavailable")?;
    if bytes.iter().all(|byte| *byte == 0) {
        return Err("Input identity unavailable".to_owned());
    }
    Ok(bytes.to_vec())
}
fn operation() -> Result<rpc::OperationId, String> {
    Ok(rpc::OperationId {
        value: Some(random::<16>()?),
    })
}
fn request<T>(body: T, credential: &str) -> Result<tonic::Request<T>, String> {
    let mut request = tonic::Request::new(body);
    request.set_timeout(std::time::Duration::from_secs(5));
    request.metadata_mut().insert(
        "x-df-local-binding",
        credential
            .parse()
            .map_err(|_| "Local binding unavailable")?,
    );
    Ok(request)
}
fn retire_transport(client: &mut Client) {
    if let Some(abort) = client.watch_abort.take() {
        abort.abort();
    }
    if let Some(connection) = client.connection.take() {
        connection.close();
    }
    client.action.take();
    client.generation += 1;
    client.busy = false;
}
fn render_join(client: &Rc<RefCell<Client>>) -> Result<(), JsValue> {
    let (document, surface, code, uncertain) = {
        let mut state = client.borrow_mut();
        clear_callbacks(&mut state);
        (
            state.document.clone(),
            state
                .root
                .query_selector(".view")?
                .ok_or_else(|| JsValue::from_str("view missing"))?,
            state.room_code.clone(),
            state.join_request.is_some(),
        )
    };
    surface.set_text_content(None);
    let art = node(&document, &surface, "img", "art", "")?;
    art.set_attribute("src", "/assets/concept-art/scene-campfire-under-stars.webp")?;
    art.set_attribute(
        "alt",
        "A campfire beneath the stars, concept artwork for your adventure",
    )?;
    let body = node(&document, &surface, "div", "scene-copy", "")?;
    node(&document, &body, "div", "eyebrow", "An adventure awaits")?;
    node(&document, &body, "h2", "", "Join your companions")?;
    node(
        &document,
        &body,
        "p",
        "narration",
        "Connect to the shared room, create your own hero, and decide what happens at the Lantern Wharf.",
    )?;
    node(&document, &body, "div", "room-code", &code)?;
    let join = button(
        client,
        &body,
        if uncertain {
            "Confirm the same join"
        } else {
            "Join this room"
        },
        join_room,
    )?;
    join.set_class_name("offered join-room");
    node(
        &document,
        &body,
        "p",
        "scope",
        "Local browser room · supported SRD 5.2.1 character options · two players",
    )?;
    node(&document, &body, "p", "feedback", "")?;
    Ok(())
}
fn join_room(client: Rc<RefCell<Client>>) {
    let prepared = (|| -> Result<_, String> {
        let mut state = client.borrow_mut();
        if state.busy {
            return Err("Waiting for the room…".to_owned());
        }
        if state.join_request.is_none() {
            state.join_request = Some(rpc::JoinRoomRequest {
                room_code: state.room_code.clone(),
                operation_id: Some(operation()?),
                join_secret: random::<32>()?,
            });
        }
        let body = state.join_request.clone().ok_or("Room proof unavailable")?;
        retire_transport(&mut state);
        state.busy = true;
        status(&state, "Joining your companions…");
        Ok((body, state.generation))
    })();
    let (body, generation) = match prepared {
        Ok(value) => value,
        Err(error) => {
            feedback(&client.borrow(), &error);
            return;
        }
    };
    wasm_bindgen_futures::spawn_local(async move {
        let run = async {
            let url = crate::browser::tunnel_url("/gameplay/tunnel")?;
            let (connection, channel) = BrowserConnection::connect(&url)
                .await
                .map_err(|_| "Room connection unavailable".to_owned())?;
            let mut rooms = RoomClient::new(channel)
                .max_decoding_message_size(8192)
                .max_encoding_message_size(8192);
            let mut request = tonic::Request::new(body);
            request.set_timeout(std::time::Duration::from_secs(5));
            let reply = rooms
                .join(request)
                .await
                .map(|reply| reply.into_inner())
                .map_err(|_| {
                    "Join outcome unknown. Confirm the same join; do not start a new one."
                        .to_owned()
                });
            connection.close();
            reply
        };
        let outcome = run.await;
        if client.borrow().generation != generation {
            return;
        }
        client.borrow_mut().busy = false;
        match outcome {
            Ok(reply) => match reply.outcome {
                Some(rpc::join_room_response::Outcome::Joined(joined)) => {
                    let original = client
                        .borrow()
                        .join_request
                        .as_ref()
                        .and_then(|request| request.operation_id.clone());
                    let valid_receipt = joined.receipt.as_ref().is_some_and(|receipt| {
                        receipt.operation_id == original
                            && receipt.session_id == joined.session_id
                            && receipt.run_id == joined.run_id
                            && matches!(
                                receipt.outcome,
                                Some(rpc::decision_receipt::Outcome::Accepted(_))
                            )
                            && receipt.revision.as_ref().is_some_and(|revision| {
                                revision.sequence.is_some() && revision.epoch.is_some()
                            })
                    });
                    let valid_scope = joined
                        .session_id
                        .as_ref()
                        .and_then(|id| id.value.as_ref())
                        .is_some_and(|bytes| bytes.len() == 16)
                        && joined
                            .run_id
                            .as_ref()
                            .and_then(|id| id.value.as_ref())
                            .is_some_and(|bytes| bytes.len() == 16);
                    if !valid_receipt
                        || !valid_scope
                        || joined.local_binding.len() != 64
                        || joined.session_id.is_none()
                        || joined.run_id.is_none()
                        || joined
                            .client_binding_id
                            .as_ref()
                            .and_then(|id| id.value.as_ref())
                            .is_none_or(|bytes| bytes.len() != 16)
                    {
                        feedback(
                            &client.borrow(),
                            "The joined scope is incomplete. Confirm the same join.",
                        );
                        return;
                    }
                    {
                        let mut state = client.borrow_mut();
                        state.credential = joined.local_binding;
                        state.session = joined.session_id;
                        state.run = joined.run_id;
                        state.binding = joined.client_binding_id;
                    }
                    connect(client);
                }
                Some(rpc::join_room_response::Outcome::Refused(refusal)) => {
                    feedback(
                        &client.borrow(),
                        match rpc::RejectionCode::try_from(refusal.code) {
                            Ok(rpc::RejectionCode::ResourceMissing) => {
                                "This two-player room is full."
                            }
                            Ok(rpc::RejectionCode::InvalidSelection) => {
                                "That room code is not available."
                            }
                            Ok(rpc::RejectionCode::StaleOffer) => {
                                "The adventure has already begun."
                            }
                            _ => "Room recovery required. Confirm the original join proof.",
                        },
                    );
                }
                None => feedback(
                    &client.borrow(),
                    "The server returned no join outcome. Confirm the same join.",
                ),
            },
            Err(error) => {
                let _ = render_join(&client);
                feedback(&client.borrow(), &error);
            }
        }
    });
}
fn connect(client: Rc<RefCell<Client>>) {
    if client.borrow().credential.is_empty() {
        let _ = render_join(&client);
        return;
    }
    let (generation, credential, session, run, binding, observed) = {
        let mut state = client.borrow_mut();
        retire_transport(&mut state);
        status(&state, "Connecting to your saved game…");
        (
            state.generation,
            state.credential.clone(),
            state.session.clone(),
            state.run.clone(),
            state.binding.clone(),
            state.revision,
        )
    };
    let (abort, registration) = AbortHandle::new_pair();
    client.borrow_mut().watch_abort = Some(abort);
    let owned = client.clone();
    wasm_bindgen_futures::spawn_local(async move {
        let run = async {
            let url = crate::browser::tunnel_url("/gameplay/tunnel")?;
            let (connection, channel) = BrowserConnection::connect(&url)
                .await
                .map_err(|_| "Game connection unavailable")?;
            if owned.borrow().generation != generation {
                connection.close();
                return Ok::<(), String>(());
            }
            let mut sessions = SessionClient::new(channel.clone())
                .max_decoding_message_size(8192)
                .max_encoding_message_size(8192);
            {
                let mut state = owned.borrow_mut();
                state.action = Some(
                    ActionClient::new(channel)
                        .max_decoding_message_size(8192)
                        .max_encoding_message_size(8192),
                );
                state.connection = Some(connection);
            }
            let body = rpc::WatchViewRequest {
                session_id: session,
                run_id: run,
                client_binding_id: binding,
                after_revision: observed,
            };
            let mut stream = sessions
                .watch(request(body, &credential)?)
                .await
                .map_err(|_| "Saved game view unavailable")?
                .into_inner();
            status(&owned.borrow(), "Connected · server owns the saved game");
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
        let result = Abortable::new(run, registration).await;
        if owned.borrow().generation == generation {
            if let Some(connection) = owned.borrow_mut().connection.take() {
                connection.close();
            }
            owned.borrow_mut().action.take();
            status(
                &owned.borrow(),
                match result {
                    Ok(Ok(())) => "Game connection closed.",
                    Ok(Err(ref error)) => error.as_str(),
                    Err(_) => "Connection cancelled.",
                },
            );
        }
    });
}
fn prepare(
    client: &Rc<RefCell<Client>>,
    offer: &rpc::GameplayActionOffer,
    character: Option<rpc::CharacterSelection>,
    savage: bool,
    graze: bool,
    destination: i32,
) -> Result<rpc::SubmitActionRequest, String> {
    let state = client.borrow();
    if state.busy || (!state.last_confirmed && state.last_request.is_some()) {
        return Err("Confirm the current operation before making a new choice.".to_owned());
    }
    Ok(rpc::SubmitActionRequest {
        session_id: state.session.clone(),
        run_id: state.run.clone(),
        observed_revision: state.revision,
        operation_id: Some(operation()?),
        offer_id: offer.offer_id.clone(),
        action_kind: offer.action_kind,
        destination,
        character,
        savage_attacker: savage,
        graze,
    })
}
fn send(client: Rc<RefCell<Client>>, body: rpc::SubmitActionRequest, replay: bool) {
    let prepared = (|| -> Result<_, String> {
        let mut state = client.borrow_mut();
        if state.busy {
            return Err("Waiting for the server…".to_owned());
        }
        let action = state
            .action
            .clone()
            .ok_or("Reconnect before sending input")?;
        if !replay {
            state.last_request = Some(body.clone());
            state.last_confirmed = false;
        }
        state.busy = true;
        feedback(&state, "Waiting for the game server…");
        Ok((action, state.credential.clone(), state.generation))
    })();
    let (mut action, credential, generation) = match prepared {
        Ok(value) => value,
        Err(error) => {
            feedback(&client.borrow(), &error);
            return;
        }
    };
    wasm_bindgen_futures::spawn_local(async move {
        let result = match request(body.clone(), &credential) {
            Ok(request) => action
                .submit(request)
                .await
                .map(|response| response.into_inner())
                .map_err(|_| "Outcome unknown. Confirm the same operation or reconnect."),
            Err(_) => Err("Input binding unavailable"),
        };
        if client.borrow().generation != generation {
            return;
        }
        let message = {
            let mut state = client.borrow_mut();
            state.busy = false;
            match result {
                Ok(response) => match response.outcome {
                    Some(rpc::submit_action_response::Outcome::CommittedDecision(receipt)) => {
                        if receipt.operation_id != body.operation_id
                            || receipt.session_id != body.session_id
                            || receipt.run_id != body.run_id
                            || receipt.revision.is_none()
                        {
                            "The receipt belongs to a different scope. Reconnect to recover."
                                .to_owned()
                        } else {
                            if state
                                .last_request
                                .as_ref()
                                .is_some_and(|saved| saved.operation_id == body.operation_id)
                            {
                                state.last_confirmed = receipt.outcome.is_some();
                            }
                            if let Some(revision) = receipt.revision {
                                let _ = state.root.set_attribute(
                                    "data-last-receipt-sequence",
                                    &revision.sequence.unwrap_or(0).to_string(),
                                );
                            }
                            let _ = state.root.set_attribute(
                                "data-last-receipt-replayed",
                                if receipt.replayed { "true" } else { "false" },
                            );
                            match receipt.outcome {
                                Some(rpc::decision_receipt::Outcome::Accepted(_))=>{
                                    if body.action_kind==rpc::GameplayActionKind::GreatswordAttack as i32 && state.first_attack.is_none() {state.first_attack=Some(body.clone());}
                                    if receipt.replayed {"Original saved outcome confirmed. No rolls or damage were repeated."} else {"Your choice is saved to the shared adventure."}.to_owned()
                                }
                                Some(rpc::decision_receipt::Outcome::Rejected(rejection))=>match rpc::RejectionCode::try_from(rejection.code) {
                                    Ok(rpc::RejectionCode::InvalidSelection)=>"The server rejected that character or action selection. Review the offered choices.",
                                    Ok(rpc::RejectionCode::WrongTurn)=>"It is another combatant's turn.",
                                    Ok(rpc::RejectionCode::StaleOffer)=>"That offer has ended. Choose from the current view.",
                                    _=>"The server declined the input. Reconnect to recover.",
                                }.to_owned(),
                                None=>"The server returned no decision outcome.".to_owned(),
                            }
                        }
                    }
                    Some(rpc::submit_action_response::Outcome::OperationObservation(_)) => {
                        "The operation requires recovery; retain the same input.".to_owned()
                    }
                    None => "The server returned no operation outcome.".to_owned(),
                },
                Err(error) => error.to_owned(),
            }
        };
        let view = client.borrow().last_view.clone();
        if let Some(view) = view {
            let _ = render(&client, view);
        }
        feedback(&client.borrow(), &message);
    });
}
fn replay(client: Rc<RefCell<Client>>, first_attack: bool) {
    let body = if first_attack {
        client.borrow().first_attack.clone()
    } else {
        client.borrow().last_request.clone()
    };
    if let Some(body) = body {
        send(client, body, true);
    }
}
fn character_render_error(
    client: &Rc<RefCell<Client>>,
    error: &df_ui::CharacterPhaseError,
) -> JsValue {
    let code = match error {
        df_ui::CharacterPhaseError::InvalidView(reason) => reason.to_string(),
        df_ui::CharacterPhaseError::Control(_) => "character-control".to_owned(),
        df_ui::CharacterPhaseError::Dom(_) => "character-dom".to_owned(),
        df_ui::CharacterPhaseError::UpdateCleanup { .. } => "character-update-cleanup".to_owned(),
    };
    let _ = client
        .borrow()
        .root
        .set_attribute("data-render-error", &code);
    JsValue::from_str("Character controls could not be displayed")
}
fn creation(
    client: &Rc<RefCell<Client>>,
    surface: &Element,
    offer: &rpc::CharacterCreationOffer,
    action: &rpc::GameplayActionOffer,
) -> Result<(), JsValue> {
    let state = client.borrow();
    let owner = state
        .binding
        .as_ref()
        .and_then(|binding| binding.value.as_ref())
        .map(|bytes| {
            bytes
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        })
        .ok_or_else(|| JsValue::from_str("character owner absent"))?;
    let editable = true;
    let view=CharacterPhaseView {generation:1,owner_key:owner,revision:state.revision.and_then(|revision|revision.sequence).ok_or_else(||JsValue::from_str("creation revision absent"))?,
        chapter:"Your hero".to_owned(),title:"A story begins with you".to_owned(),description:offer.description.clone(),
        connection:"Connected to your shared room".to_owned(),status:CharacterStatus::Editing,
        status_message:"Choose one option in every group, then confirm. Character names use letters, dashes or underscores (up to 24 characters).".to_owned(),
        editable,name:String::new(),flavor:String::new(),appearance:None,portrait:None,
        groups:offer.groups.iter().map(|group|CharacterGroup {id:group.group_id.clone(),label:group.label.clone(),
            description:"Choose one offered option.".to_owned(),selected:None,options:group.options.iter().map(|option|CharacterOption {
                id:option.option_id.clone(),label:option.label.clone(),description:if option.description.trim().is_empty() {option.label.clone()} else {option.description.clone()},
                enabled:true,availability:"Available in this room".to_owned(),portrait:None,
            }).collect()}).collect(),
        facts:vec![CharacterFact {label:"Supported rules".to_owned(),value:offer.source_revision.clone()}],
        actions:vec![CharacterAction {id:action.offer_id.clone(),kind:CharacterActionKind::SubmitDraft,
            label:"Confirm your hero".to_owned(),enabled:editable}],
    };
    drop(state);
    if let Some(existing) = client.borrow().creation.as_ref() {
        existing
            .update(&view)
            .map_err(|error| character_render_error(client, &error))?;
        return Ok(());
    }
    let weak = Rc::downgrade(client);
    let offered = action.clone();
    let created = CharacterPhaseSurface::create(
        &client.borrow().document,
        "journey-character",
        &view,
        CharacterLimits {
            max_groups: 16,
            max_options: 128,
            max_facts: 32,
            max_actions: 4,
            max_text_bytes: 8192,
        },
        move |submission| {
            let Some(client) = weak.upgrade() else {
                return;
            };
            let selection = rpc::CharacterSelection {
                name: submission.name,
                choices: submission
                    .choices
                    .into_iter()
                    .map(|choice| rpc::JourneyChoice {
                        group_id: choice.group_id,
                        option_id: choice.option_id,
                    })
                    .collect(),
            };
            match prepare(
                &client,
                &offered,
                Some(selection),
                false,
                false,
                rpc::HarborDestination::Unspecified as i32,
            ) {
                Ok(body) => send(client, body, false),
                Err(error) => feedback(&client.borrow(), &error),
            }
        },
    )
    .map_err(|error| character_render_error(client, &error))?;
    // Flavor is not an admitted field in this bounded character contract.
    created.flavor_input().root().set_attribute("hidden", "")?;
    surface.append_child(created.root())?;
    client.borrow_mut().creation = Some(created);
    Ok(())
}
fn action_button(
    client: &Rc<RefCell<Client>>,
    parent: &Element,
    offer: rpc::GameplayActionOffer,
) -> Result<(), JsValue> {
    let document = client.borrow().document.clone();
    let mut inputs = Vec::new();
    if offer.input_groups.len() > 2 {
        return Err(JsValue::from_str("offered inputs exceed bound"));
    }
    for group in &offer.input_groups {
        if group.options.len() != 2 {
            return Err(JsValue::from_str("unsupported offered input shape"));
        }
        let label = node(&document, parent, "label", "attack-option", &group.label)?;
        let select = node(&document, &label, "select", "", "")?.dyn_into::<HtmlSelectElement>()?;
        for option in &group.options {
            let item = node(&document, select.as_ref(), "option", "", &option.label)?;
            item.set_attribute("value", &option.option_id)?;
        }
        inputs.push((group.group_id.clone(), select));
    }
    if offer.destinations.len() > 4 {
        return Err(JsValue::from_str("offered destinations exceed bound"));
    }
    let destination = if offer.destinations.is_empty() {
        None
    } else {
        let label = node(&document, parent, "label", "attack-option", "Destination")?;
        let select = node(&document, &label, "select", "", "")?.dyn_into::<HtmlSelectElement>()?;
        select.set_attribute("data-input", "destination")?;
        for option in &offer.destinations {
            let item = node(&document, select.as_ref(), "option", "", &option.label)?;
            item.set_attribute("value", &option.destination.to_string())?;
        }
        Some(select)
    };
    let offered = offer.clone();
    let button = button(client, parent, &offer.label, move |client| {
        let mut savage = false;
        let mut graze = false;
        for (key, input) in &inputs {
            let value = input.value();
            if !matches!(value.as_str(), "yes" | "no") {
                feedback(&client.borrow(), "Choose an offered attack approach.");
                return;
            }
            match key.as_str() {
                "savage-attacker" => savage = value == "yes",
                "graze" => graze = value == "yes",
                _ => {
                    feedback(
                        &client.borrow(),
                        "The server offered an unsupported required input.",
                    );
                    return;
                }
            }
        }
        let selected = if let Some(select) = &destination {
            let Ok(selected) = select.value().parse::<i32>() else {
                feedback(&client.borrow(), "Choose an offered destination.");
                return;
            };
            if !offered
                .destinations
                .iter()
                .any(|option| option.destination == selected)
            {
                feedback(&client.borrow(), "Choose an offered destination.");
                return;
            }
            selected
        } else {
            rpc::HarborDestination::Unspecified as i32
        };
        match prepare(&client, &offered, None, savage, graze, selected) {
            Ok(body) => send(client, body, false),
            Err(error) => feedback(&client.borrow(), &error),
        }
    })?;
    button.set_class_name("offered");
    button.set_attribute("data-action-kind", &offer.action_kind.to_string())?;
    if client.borrow().busy
        || (!client.borrow().last_confirmed && client.borrow().last_request.is_some())
    {
        button.set_attribute("disabled", "")?;
    }
    Ok(())
}
fn render(client: &Rc<RefCell<Client>>, view: rpc::ViewMessage) -> Result<(), JsValue> {
    let previous = client.borrow().revision;
    let next = view
        .revision
        .ok_or_else(|| JsValue::from_str("server revision missing"))?;
    if previous.is_some_and(|old| old.epoch == next.epoch && old.sequence > next.sequence) {
        return Ok(());
    }
    let (document, root, role) = {
        let mut state = client.borrow_mut();
        clear_callbacks(&mut state);
        state.revision = Some(next);
        state.last_view = Some(view.clone());
        (state.document.clone(), state.root.clone(), state.role)
    };
    let (narration, scene, journey, offers, clue) = match view.audience {
        Some(rpc::view_message::Audience::Player(player)) if role == "player" => (
            player.narration,
            player.scene,
            player.journey,
            player.offers,
            player.private_clue,
        ),
        Some(rpc::view_message::Audience::Display(display)) if role == "display" => (
            display.narration,
            display.scene,
            display.journey,
            Vec::new(),
            String::new(),
        ),
        _ => return Err(JsValue::from_str("server audience mismatch")),
    };
    let journey = journey.ok_or_else(|| JsValue::from_str("server journey missing"))?;
    if journey.party.len() > 2 || offers.len() > 4 {
        return Err(JsValue::from_str("server view exceeds bound"));
    }
    let surface = root
        .query_selector(".view")?
        .ok_or_else(|| JsValue::from_str("view absent"))?;
    root.set_attribute("data-phase", &journey.phase.to_string())?;
    if let Some(creation_offer) = journey.creation.as_ref() {
        let offered = offers
            .iter()
            .find(|offer| offer.action_kind == rpc::GameplayActionKind::CreateCharacter as i32)
            .ok_or_else(|| JsValue::from_str("character action missing"))?;
        if client.borrow().creation.is_none() {
            surface.set_text_content(None);
        }
        creation(client, &surface, creation_offer, offered)?;
        // Recovery remains reachable while a creation submission is uncertain.
        let tools = if let Some(tools) = surface.query_selector(".tools")? {
            tools
        } else {
            node(&document, &surface, "div", "tools", "")?
        };
        tools.set_text_content(None);
        button(client, &tools, "Reconnect", connect)?;
        if client.borrow().last_request.is_some() {
            button(client, &tools, "Confirm saved choice", |client| {
                replay(client, false)
            })?;
        }
        if surface.query_selector(".feedback")?.is_none() {
            node(&document, &surface, "p", "feedback", "")?;
        }
        return Ok(());
    }
    if let Some(creation) = client.borrow_mut().creation.take() {
        creation
            .dispose()
            .map_err(|_| JsValue::from_str("character retirement failed"))?;
    }
    surface.set_text_content(None);
    let scene = scene.ok_or_else(|| JsValue::from_str("server scene missing"))?;
    if !matches!(
        scene.scene_asset.as_str(),
        "assets/ui/scenes/mara-harbor-v4.png"
            | "assets/concept-art/scene-tavern-barkeep-talk-rain.webp"
    ) {
        return Err(JsValue::from_str("unknown required scene asset"));
    }
    let art = node(&document, &surface, "img", "art", "")?;
    art.set_attribute("src", &format!("/{}", scene.scene_asset))?;
    art.set_attribute("alt", "Authored scene concept illustration")?;
    surface.set_attribute("data-destination", &scene.destination.to_string())?;
    let body = node(&document, &surface, "div", "scene-copy", "")?;
    node(
        &document,
        &body,
        "div",
        "eyebrow",
        if role == "display" {
            "Your shared adventure"
        } else {
            "Your story · your choices"
        },
    )?;
    node(
        &document,
        &body,
        if role == "display" { "h1" } else { "h2" },
        "",
        &scene.title,
    )?;
    node(&document, &body, "p", "narration", &narration)?;
    let phase = rpc::JourneyPhase::try_from(journey.phase)
        .map_err(|_| JsValue::from_str("unknown required phase"))?;
    if phase == rpc::JourneyPhase::Room {
        let share = node(&document, &body, "div", "room-share", "")?;
        node(&document, &share, "span", "scope", "Join this adventure")?;
        node(&document, &share, "div", "room-code", &journey.room_code)?;
        let link = node(&document, &share, "a", "join-link", "Open player client")?;
        if !journey.join_path.starts_with("/gameplay/player?room=") {
            return Err(JsValue::from_str("join route invalid"));
        }
        link.set_attribute("href", &journey.join_path)?;
        link.set_attribute("target", "_blank")?;
    }
    let party = node(&document, &body, "div", "party", "")?;
    for member in &journey.party {
        node(
            &document,
            &party,
            "div",
            "party-member",
            &format!(
                "{} · {}",
                member.name,
                if member.character_ready {
                    "Ready"
                } else {
                    "Creating"
                }
            ),
        )?;
    }
    if !journey.dialogue.is_empty() {
        let speech = node(&document, &body, "div", "dialogue", "")?;
        let portrait = node(&document, &speech, "img", "speaker-portrait", "")?;
        portrait.set_attribute("src", "/assets/concept-art/vell-avatar.webp")?;
        portrait.set_attribute("alt", "Concept portrait illustrating the speaker")?;
        node(&document, &speech, "span", "eyebrow", &journey.speaker)?;
        node(&document, &speech, "blockquote", "", &journey.dialogue)?;
    }
    if !clue.is_empty() {
        node(&document, &body, "p", "clue", &clue)?;
    }
    if let Some(sheet) = journey.own_character {
        let summary = node(&document, &body, "div", "hero-summary", "")?;
        node(&document, &summary, "strong", "", &sheet.name)?;
        node(
            &document,
            &summary,
            "span",
            "",
            &format!(
                "Dwarf Fighter · HP {}/{} · AC {}",
                sheet.hit_points, sheet.maximum_hit_points, sheet.armor_class
            ),
        )?;
        let facts = node(&document, &summary, "details", "sheet-facts", "")?;
        node(
            &document,
            &facts,
            "summary",
            "",
            "Your accepted character sheet",
        )?;
        for fact in &sheet.facts {
            node(
                &document,
                &facts,
                "p",
                "",
                &format!("{} · {}", fact.label, fact.value),
            )?;
        }
        if let Some(uses) = sheet
            .facts
            .iter()
            .find(|fact| fact.label == "Second Wind uses")
        {
            node(
                &document,
                &summary,
                "span",
                "wind-uses",
                &format!("Second Wind · {} uses remaining", uses.value),
            )?;
        }
    }
    if let Some(combat) = journey.combat {
        let battle = node(&document, &body, "section", "battle", "")?;
        node(
            &document,
            &battle,
            "div",
            "eyebrow",
            &format!(
                "Round {} · {}",
                combat.round,
                if combat.finished {
                    "Encounter resolved".to_owned()
                } else {
                    format!("{}'s turn", combat.active_actor)
                }
            ),
        )?;
        let roster = node(&document, &battle, "div", "combat-roster", "")?;
        for member in &combat.participants {
            let card = node(
                &document,
                &roster,
                "div",
                if member.active_turn {
                    "combatant active"
                } else {
                    "combatant"
                },
                "",
            )?;
            node(&document, &card, "strong", "", &member.name)?;
            node(
                &document,
                &card,
                "span",
                "",
                &format!(
                    "HP {}/{}{}",
                    member.hit_points,
                    member.maximum_hit_points,
                    if member.unconscious {
                        " · Unconscious"
                    } else {
                        ""
                    }
                ),
            )?;
            let hp = node(&document, &card, "progress", "hp", "")?;
            hp.set_attribute("max", &member.maximum_hit_points.to_string())?;
            hp.set_attribute("value", &member.hit_points.to_string())?;
        }
        let history = node(&document, &battle, "div", "combat-history", "")?;
        for outcome in &combat.outcomes {
            let result = node(&document, &history, "article", "combat-result", "")?;
            node(
                &document,
                &result,
                "span",
                "roll-number",
                &outcome.attack_die.to_string(),
            )?;
            node(
                &document,
                &result,
                "strong",
                "",
                &format!("{} → {}", outcome.actor_name, outcome.target_name),
            )?;
            node(
                &document,
                &result,
                "p",
                "",
                &format!(
                    "{} + {} = {} vs AC {} · {}{} · {} damage · {} HP remaining",
                    outcome.attack_die,
                    outcome.attack_modifier,
                    outcome.attack_total,
                    outcome.target_armor_class,
                    if outcome.hit {
                        "Hit"
                    } else if outcome.grazed {
                        "Graze"
                    } else {
                        "Miss"
                    },
                    if outcome.critical { " · Critical" } else { "" },
                    outcome.damage,
                    outcome.target_hit_points
                ),
            )?;
            if outcome.knocked_out {
                node(
                    &document,
                    &result,
                    "p",
                    "clue",
                    "Nonlethal knockout · Unconscious with 1 HP",
                )?;
            }
        }
    }
    let actions = node(&document, &body, "div", "scene-actions", "")?;
    for offer in offers {
        action_button(client, &actions, offer)?;
    }
    let tools = node(&document, &body, "div", "tools", "")?;
    button(client, &tools, "Reconnect", connect)?;
    if client.borrow().last_request.is_some() {
        button(client, &tools, "Confirm saved choice", |client| {
            replay(client, false)
        })?;
    }
    if client.borrow().first_attack.is_some() {
        button(client, &tools, "Confirm original strike", |client| {
            replay(client, true)
        })?;
    }
    node(&document, &body, "p", "feedback", "")?;
    Ok(())
}
pub(super) fn start() -> Result<(), JsValue> {
    let document = web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| JsValue::from_str("document absent"))?;
    for role in ["player", "display"] {
        let Some(root) = document.get_element_by_id(role) else {
            continue;
        };
        let credential = root.get_attribute("data-local-binding").unwrap_or_default();
        root.remove_attribute("data-local-binding")?;
        let room_code = root
            .get_attribute("data-room")
            .unwrap_or_else(|| "LANTERN".to_owned());
        let client = Rc::new(RefCell::new(Client {
            document: document.clone(),
            root,
            role,
            credential,
            room_code,
            session: if role == "display" {
                Some(rpc::SessionId {
                    value: Some(vec![0x41; 16]),
                })
            } else {
                None
            },
            run: if role == "display" {
                Some(rpc::RunId {
                    value: Some(vec![0x42; 16]),
                })
            } else {
                None
            },
            binding: if role == "display" {
                Some(rpc::ClientBindingId {
                    value: Some(vec![0x72; 16]),
                })
            } else {
                None
            },
            generation: 0,
            revision: None,
            last_view: None,
            join_request: None,
            last_request: None,
            first_attack: None,
            last_confirmed: false,
            action: None,
            connection: None,
            watch_abort: None,
            callbacks: Vec::new(),
            creation: None,
            busy: false,
        }));
        CLIENTS.with(|clients| clients.borrow_mut().push(client.clone()));
        if role == "player" {
            render_join(&client)?;
        } else {
            connect(client);
        }
    }
    Ok(())
}
