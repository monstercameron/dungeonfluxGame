#[cfg(target_arch = "wasm32")]
#[path = "../support/browser_pcm.rs"]
mod browser_pcm;

#[cfg(target_arch = "wasm32")]
mod browser {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use df_audio::browser_playback::{
        BrowserPlayback, PlaybackError, PlaybackLimits, PlaybackSnapshot, PlaybackStart,
        PlaybackState,
    };
    use df_audio::{AudioReceipt, QueueError, QueueState};
    use df_media::schedule::ScheduleLimits;
    use df_media::speech::{
        SpeechIdentity, SpeechLimits, SpeechScheduler, SpeechStopReason, SpeechStopped,
    };
    use df_model::checkpoint::JobId;
    use wasm_bindgen::{JsCast, JsValue, closure::Closure, prelude::wasm_bindgen};
    use web_sys::{Element, Event};

    use super::browser_pcm::{self, FRAMES, LocalPcm};

    // Private fixture bindings only. Proxy supplies the genuine method receiver
    // to Rust; no JavaScript application body or production fault hook is added.
    #[wasm_bindgen]
    extern "C" {
        #[wasm_bindgen(js_namespace = Reflect, js_name = get, catch)]
        fn property_get(target: &JsValue, name: &JsValue) -> Result<JsValue, JsValue>;
        #[wasm_bindgen(js_namespace = Reflect, js_name = set, catch)]
        fn property_set(target: &JsValue, name: &JsValue, value: &JsValue)
        -> Result<bool, JsValue>;
        #[wasm_bindgen(js_namespace = Reflect, js_name = apply, catch)]
        fn native_apply(
            target: &JsValue,
            receiver: &JsValue,
            args: &JsValue,
        ) -> Result<JsValue, JsValue>;
        #[wasm_bindgen(js_namespace = Object, js_name = create, catch)]
        fn object_create(prototype: &JsValue) -> Result<JsValue, JsValue>;
        #[wasm_bindgen(js_namespace = Object, js_name = getOwnPropertyDescriptor, catch)]
        fn own_descriptor(target: &JsValue, name: &JsValue) -> Result<JsValue, JsValue>;
        #[wasm_bindgen(js_namespace = Object, js_name = defineProperty, catch)]
        fn property_define(
            target: &JsValue,
            name: &JsValue,
            descriptor: &JsValue,
        ) -> Result<JsValue, JsValue>;
        #[wasm_bindgen(js_name = Proxy)]
        type StopProxy;
        #[wasm_bindgen(constructor, js_class = Proxy, catch)]
        fn new(target: &JsValue, handler: &JsValue) -> Result<StopProxy, JsValue>;
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum StopFaultObservation {
        Pending,
        NativeFailed,
        InvalidReturn,
        StoppedThenThrew,
    }

    struct StopFault {
        prototype: JsValue,
        descriptor: JsValue,
        _proxy: StopProxy,
        _callback: Closure<dyn FnMut(JsValue, JsValue, JsValue)>,
        observed: Rc<Cell<StopFaultObservation>>,
    }

    impl StopFault {
        fn install() -> Result<Self, JsValue> {
            let window = web_sys::window().ok_or_else(|| JsValue::from_str("window absent"))?;
            let constructor = property_get(
                window.as_ref(),
                &JsValue::from_str("AudioScheduledSourceNode"),
            )?;
            let prototype = property_get(&constructor, &JsValue::from_str("prototype"))?;
            let descriptor = own_descriptor(&prototype, &JsValue::from_str("stop"))?;
            if !descriptor.is_object() {
                return Err(JsValue::from_str(
                    "own stop descriptor absent; fault not installed",
                ));
            }
            let original = property_get(&descriptor, &JsValue::from_str("value"))?;
            if !original.is_function() {
                return Err(JsValue::from_str(
                    "stop is not an own data function; fault not installed",
                ));
            }
            let observed = Rc::new(Cell::new(StopFaultObservation::Pending));
            let flag = Rc::clone(&observed);
            let callback = Closure::<dyn FnMut(JsValue, JsValue, JsValue)>::wrap(Box::new(
                move |target: JsValue, receiver: JsValue, args: JsValue| {
                    let result = native_apply(&target, &receiver, &args);
                    // throw_val/throw_str do not unwind Rust locals. Release every
                    // callback argument before throwing; the outer fixture owns Closure.
                    drop(args);
                    drop(receiver);
                    drop(target);
                    match result {
                        Ok(value) => {
                            let valid = value.is_undefined();
                            drop(value);
                            flag.set(if valid {
                                StopFaultObservation::StoppedThenThrew
                            } else {
                                StopFaultObservation::InvalidReturn
                            });
                            wasm_bindgen::throw_str(
                                "fixture: native stop ran before controlled failure",
                            );
                        }
                        Err(error) => {
                            flag.set(StopFaultObservation::NativeFailed);
                            wasm_bindgen::throw_val(error);
                        }
                    }
                },
            ));
            let handler = object_create(&JsValue::NULL)?;
            if !property_set(&handler, &JsValue::from_str("apply"), callback.as_ref())? {
                return Err(JsValue::from_str("fault handler not installed"));
            }
            let proxy = StopProxy::new(&original, &handler)?;
            let replacement = object_create(&JsValue::NULL)?;
            let proxy_value: &JsValue = proxy.as_ref();
            for name in ["value", "writable", "enumerable", "configurable"] {
                let key = JsValue::from_str(name);
                let value = if name == "value" {
                    proxy_value.clone()
                } else {
                    property_get(&descriptor, &key)?
                };
                if !property_set(&replacement, &key, &value)? {
                    return Err(JsValue::from_str("fault descriptor not prepared"));
                }
            }
            property_define(&prototype, &JsValue::from_str("stop"), &replacement)?;
            Ok(Self {
                prototype,
                descriptor,
                _proxy: proxy,
                _callback: callback,
                observed,
            })
        }

        fn restore(&self) -> Result<(), JsValue> {
            property_define(
                &self.prototype,
                &JsValue::from_str("stop"),
                &self.descriptor,
            )?;
            let restored = own_descriptor(&self.prototype, &JsValue::from_str("stop"))?;
            for name in ["value", "writable", "enumerable", "configurable"] {
                let key = JsValue::from_str(name);
                if property_get(&restored, &key)? != property_get(&self.descriptor, &key)? {
                    return Err(JsValue::from_str("stop descriptor restoration mismatch"));
                }
            }
            Ok(())
        }
    }

    struct ButtonListener {
        element: Element,
        on_click: Closure<dyn FnMut(Event)>,
    }

    struct Fixture {
        playback: BrowserPlayback,
        source: Option<LocalPcm>,
        receipt: Option<AudioReceipt>,
        old_receipt: Option<AudioReceipt>,
        media_stop_before: Option<PlaybackSnapshot>,
        stop_fault: Option<StopFault>,
        replacement_fault_before: Option<PlaybackSnapshot>,
        generation: u64,
        root: Element,
        status: Element,
        buttons: Vec<ButtonListener>,
    }

    thread_local! {
        static FIXTURE: RefCell<Option<Fixture>> = const { RefCell::new(None) };
    }

    fn error(value: impl std::fmt::Debug) -> JsValue {
        JsValue::from_str(&format!("{value:?}"))
    }

    fn take() -> Result<Fixture, JsValue> {
        FIXTURE
            .with(|cell| cell.borrow_mut().take())
            .ok_or_else(|| JsValue::from_str("fixture busy or absent"))
    }

    fn restore(fixture: Fixture) {
        FIXTURE.with(|cell| *cell.borrow_mut() = Some(fixture));
    }

    fn media_stopped(identity: SpeechIdentity) -> Result<SpeechStopped, JsValue> {
        // Actual local media dispatch/stop facts. The opaque command is not a
        // speech codec; the verified square-tone PCM fixture remains independent.
        let mut producer = SpeechScheduler::new(
            identity.basis,
            ScheduleLimits {
                queue_items: 1,
                queue_bytes: 1,
                speech_items: 1,
                speech_bytes: 1,
                execution_slots: 1,
                speech_slots: 1,
            },
            SpeechLimits {
                maximum_chunks: 1,
                maximum_bytes: 1,
            },
        )
        .map_err(error)?;
        producer.admit(identity, Box::from([9_u8])).map_err(error)?;
        let dispatch = producer
            .begin()
            .map_err(error)?
            .ok_or_else(|| JsValue::from_str("media dispatch absent"))?;
        producer
            .stop(&dispatch, SpeechStopReason::Cancelled)
            .map_err(error)
    }

    fn unchanged_resources(before: PlaybackSnapshot, after: PlaybackSnapshot) -> bool {
        before.state == after.state
            && before.queue == after.queue
            && before.sources == after.sources
            && before.ended_callbacks == after.ended_callbacks
            && before.pending_promises == after.pending_promises
            && before.retained_sample_bytes == after.retained_sample_bytes
            && before.peak_sample_bytes == after.peak_sample_bytes
            && before.completed_buffers == after.completed_buffers
            && before.stopped_buffers == after.stopped_buffers
            && before.scheduled_start_seconds == after.scheduled_start_seconds
            && before.scheduling_lateness_seconds == after.scheduling_lateness_seconds
            && before.unlock_refused == after.unlock_refused
    }

    impl Fixture {
        fn restore_stop_fault(&mut self) -> Result<(), JsValue> {
            if let Some(fault) = &self.stop_fault {
                fault.restore()?;
            }
            // A restoration error leaves this bounded owner and callback alive.
            // Never drop a Rust closure while the prototype might reference it.
            self.stop_fault = None;
            Ok(())
        }

        fn render(&self, outcome: &str) {
            self.status.set_text_content(Some(&format!(
                "{outcome}\n{:?}\nLocal PCM tone only. Audible observation is an independent gate.",
                self.playback.snapshot(),
            )));
        }

        fn prepare(&mut self, channels: u16) -> Result<(), PlaybackError> {
            let source = browser_pcm::local_pcm(channels);
            source
                .cache
                .lease_bytes(&source.source)
                .map_err(|_| PlaybackError::Queue(QueueError::WrongAsset))?;
            self.generation += 1;
            let replacement = self.playback.replace(
                &browser_pcm::lease(),
                browser_pcm::basis(),
                browser_pcm::identity(self.generation),
                source.reference.clone(),
                source
                    .buffer
                    .as_ref()
                    .ok_or(PlaybackError::BrowserOperation)?
                    .format(),
                PlaybackStart {
                    first_frame: 48_000,
                    lead_seconds: 0.05,
                },
            )?;
            let (receipt, stop_result) = replacement.into_parts();
            self.old_receipt = self.receipt.take();
            self.receipt = Some(receipt);
            self.source = Some(source);
            stop_result
        }

        fn admit(&mut self) -> Result<(), PlaybackError> {
            let source = self
                .source
                .as_mut()
                .ok_or(PlaybackError::BrowserOperation)?;
            source
                .cache
                .lease_bytes(&source.source)
                .map_err(|_| PlaybackError::Queue(QueueError::WrongAsset))?;
            let receipt = self
                .receipt
                .as_ref()
                .ok_or(PlaybackError::BrowserOperation)?;
            if let Some(buffer) = source.buffer.take() {
                if let Err(refusal) = self.playback.enqueue(
                    receipt,
                    &browser_pcm::lease(),
                    source.manifest,
                    0,
                    48_000,
                    buffer,
                ) {
                    source.buffer = Some(refusal.buffer);
                    return Err(PlaybackError::Queue(refusal.reason));
                }
                self.playback
                    .close(receipt, &browser_pcm::lease(), 1, 48_000 + FRAMES)?;
            }
            self.playback.pump(&browser_pcm::lease())
        }

        fn action(&mut self, action: &str) -> Result<(), PlaybackError> {
            self.restore_stop_fault()
                .map_err(|_| PlaybackError::BrowserOperation)?;
            match action {
                "unlock" => self.playback.request_unlock(&browser_pcm::lease()),
                "mono" => {
                    self.prepare(1)?;
                    self.admit()
                }
                "stereo" => {
                    self.prepare(2)?;
                    self.admit()
                }
                "pump" => {
                    self.playback.pump(&browser_pcm::lease())?;
                    if self
                        .source
                        .as_ref()
                        .is_some_and(|source| source.buffer.is_some())
                    {
                        self.admit()?;
                    }
                    Ok(())
                }
                "cancel" => self.playback.cancel(
                    self.receipt
                        .as_ref()
                        .ok_or(PlaybackError::BrowserOperation)?,
                    &browser_pcm::lease(),
                ),
                "replace" => self.prepare(2),
                "revoke" => {
                    if let Some(source) = &mut self.source {
                        let scope = df_client::cache::CacheScope {
                            session: browser_pcm::basis().session,
                            run: browser_pcm::basis().run,
                            binding: browser_pcm::binding(),
                        };
                        let revision = browser_pcm::basis()
                            .revision
                            .next_sequence()
                            .map_err(|_| PlaybackError::BrowserOperation)?;
                        source
                            .cache
                            .apply_current(scope, revision, &[])
                            .map_err(|_| PlaybackError::BrowserOperation)?;
                    }
                    self.playback.revoke()
                }
                "dispose" => self.playback.dispose(),
                _ => Err(PlaybackError::BrowserOperation),
            }
        }
    }

    #[wasm_bindgen(start)]
    pub fn mount() -> Result<(), JsValue> {
        let window = web_sys::window().ok_or_else(|| JsValue::from_str("window absent"))?;
        let document = window
            .document()
            .ok_or_else(|| JsValue::from_str("document absent"))?;
        let body = document
            .body()
            .ok_or_else(|| JsValue::from_str("body absent"))?;
        let root = document.create_element("main")?;
        let title = document.create_element("h1")?;
        title.set_text_content(Some("DungeonFlux local PCM playback fixture"));
        root.append_child(&title)?;
        let instructions = document.create_element("p")?;
        instructions.set_text_content(Some(
            "Click Unlock audio, then await fixture_wait_unlock. Play mono/stereo, then Pump after actual ended. Replace while active, Pump to collect old end, then Pump again to admit new PCM. Dispose and await fixture_wait_closed before unmounting. No speech provider is used.",
        ));
        root.append_child(&instructions)?;
        let status = document.create_element("pre")?;
        status.set_attribute("id", "pcm-status")?;
        root.append_child(&status)?;
        let playback = BrowserPlayback::new(
            browser_pcm::queue(),
            browser_pcm::lease(),
            PlaybackLimits {
                total_sample_bytes: 1024 * 1024,
            },
        )
        .map_err(error)?;
        let mut fixture = Fixture {
            playback,
            source: None,
            receipt: None,
            old_receipt: None,
            media_stop_before: None,
            stop_fault: None,
            replacement_fault_before: None,
            generation: 0,
            root: root.clone(),
            status,
            buttons: Vec::new(),
        };
        for (action, label) in [
            ("unlock", "Unlock audio"),
            ("mono", "Play mono PCM"),
            ("stereo", "Play stereo PCM"),
            ("pump", "Pump actual ended"),
            ("cancel", "Cancel current"),
            ("replace", "Replace with stereo"),
            ("revoke", "Revoke source and output"),
            ("dispose", "Dispose context"),
        ] {
            let button = document.create_element("button")?;
            button.set_text_content(Some(label));
            button.set_attribute("data-action", action)?;
            let callback = Closure::wrap(Box::new(move |_event: Event| {
                FIXTURE.with(|cell| {
                    if let Some(fixture) = cell.borrow_mut().as_mut() {
                        let outcome = fixture.action(action);
                        fixture.render(&format!("{action}: {outcome:?}"));
                    }
                });
            }) as Box<dyn FnMut(Event)>);
            button.add_event_listener_with_callback("click", callback.as_ref().unchecked_ref())?;
            root.append_child(&button)?;
            fixture.buttons.push(ButtonListener {
                element: button,
                on_click: callback,
            });
        }
        body.append_child(&root)?;
        fixture.render("Mounted, unlock required");
        restore(fixture);
        Ok(())
    }

    #[wasm_bindgen]
    pub async fn fixture_wait_unlock() -> Result<(), JsValue> {
        let mut fixture = take()?;
        let result = fixture.playback.wait_unlocked().await;
        fixture.render(&format!("Unlock settled: {result:?}"));
        restore(fixture);
        result.map_err(error)
    }

    #[wasm_bindgen]
    pub fn fixture_snapshot() -> Result<String, JsValue> {
        FIXTURE.with(|cell| {
            let fixture = cell.borrow();
            let fixture = fixture
                .as_ref()
                .ok_or_else(|| JsValue::from_str("fixture busy"))?;
            Ok(format!("{:?}", fixture.playback.snapshot()))
        })
    }

    #[wasm_bindgen]
    pub fn fixture_assert_refused_before_unlock() -> Result<(), JsValue> {
        FIXTURE.with(|cell| {
            let mut fixture = cell.borrow_mut();
            let fixture = fixture
                .as_mut()
                .ok_or_else(|| JsValue::from_str("fixture busy"))?;
            let result = fixture.prepare(1);
            if !matches!(
                result,
                Err(PlaybackError::UnlockRequired | PlaybackError::UnlockPending)
            ) {
                return Err(error(result));
            }
            if fixture.playback.snapshot().sources != 0 {
                return Err(JsValue::from_str("locked operation created a source"));
            }
            Ok(())
        })
    }

    #[wasm_bindgen]
    pub fn fixture_assert_old_decode_refused() -> Result<(), JsValue> {
        FIXTURE.with(|cell| {
            let mut fixture = cell.borrow_mut();
            let fixture = fixture
                .as_mut()
                .ok_or_else(|| JsValue::from_str("fixture busy"))?;
            let old = fixture
                .old_receipt
                .as_ref()
                .ok_or_else(|| JsValue::from_str("replace first"))?;
            let mut source = browser_pcm::local_pcm(1);
            let result = fixture.playback.enqueue(
                old,
                &browser_pcm::lease(),
                source.manifest,
                1,
                48_000 + FRAMES,
                source
                    .buffer
                    .take()
                    .ok_or_else(|| JsValue::from_str("fixture buffer absent"))?,
            );
            if !matches!(result, Err(ref refusal) if refusal.reason == QueueError::StaleReceipt) {
                return Err(error(result));
            }
            Ok(())
        })
    }

    #[wasm_bindgen]
    pub fn fixture_assert_cancel_retains_until_terminal() -> Result<(), JsValue> {
        FIXTURE.with(|cell| {
            let mut fixture = cell.borrow_mut();
            let fixture = fixture
                .as_mut()
                .ok_or_else(|| JsValue::from_str("fixture busy"))?;
            fixture.action("cancel").map_err(error)?;
            let snapshot = fixture.playback.snapshot();
            if snapshot.queue.state != QueueState::WaitingForStop
                || snapshot.sources != 1
                || snapshot.retained_sample_bytes == 0
            {
                return Err(error(snapshot));
            }
            fixture.render("Stop requested; exact old PCM retained until actual terminal");
            Ok(())
        })
    }

    #[wasm_bindgen]
    pub fn fixture_assert_replacement_waits_for_old_end() -> Result<(), JsValue> {
        FIXTURE.with(|cell| {
            let mut fixture = cell.borrow_mut();
            let fixture = fixture
                .as_mut()
                .ok_or_else(|| JsValue::from_str("fixture busy"))?;
            fixture.prepare(2).map_err(error)?;
            let result = fixture.admit();
            if result != Err(PlaybackError::Queue(QueueError::WaitingForStop)) {
                return Err(error(result));
            }
            let snapshot = fixture.playback.snapshot();
            if snapshot.sources != 1 || snapshot.queue.state != QueueState::WaitingForStop {
                return Err(error(snapshot));
            }
            fixture.render("Replacement waits for actual old ended; no new source released");
            Ok(())
        })
    }

    #[wasm_bindgen]
    pub fn fixture_assert_media_stop_receipt_refusals() -> Result<(), JsValue> {
        FIXTURE.with(|cell| {
            let mut fixture = cell.borrow_mut();
            let fixture = fixture
                .as_mut()
                .ok_or_else(|| JsValue::from_str("fixture busy"))?;
            let initial = fixture.playback.snapshot();
            if initial.state != PlaybackState::Ready
                || initial.sources != 0
                || initial.queue.retained_buffers != 0
                || initial.pending_promises != 0
            {
                return Err(JsValue::from_str(
                    "unlock and settle existing sources before media-stop probe",
                ));
            }
            fixture.media_stop_before = None;
            fixture.prepare(1).map_err(error)?;
            let original = fixture
                .receipt
                .clone()
                .ok_or_else(|| JsValue::from_str("original receipt absent"))?;
            let delayed = media_stopped(original.identity())?;
            let source = fixture
                .source
                .as_ref()
                .ok_or_else(|| JsValue::from_str("local PCM absent"))?;
            let reference = source.reference.clone();
            let format = source
                .buffer
                .as_ref()
                .ok_or_else(|| JsValue::from_str("local PCM buffer absent"))?
                .format();
            let mut middle = original.identity();
            middle.job = JobId::from_bytes(&[8; 16]).map_err(error)?;
            // A and B have no dispatched samples. This synchronous replacement
            // needs no timer or invented ended event; fresh A owns the real source.
            let middle_replacement = fixture
                .playback
                .replace(
                    &browser_pcm::lease(),
                    browser_pcm::basis(),
                    middle,
                    reference.clone(),
                    format,
                    PlaybackStart {
                        first_frame: 48_000,
                        lead_seconds: 0.05,
                    },
                )
                .map_err(error)?;
            let (middle_receipt, middle_stop_result) = middle_replacement.into_parts();
            fixture.receipt = Some(middle_receipt);
            middle_stop_result.map_err(error)?;
            let replacement = fixture
                .playback
                .replace(
                    &browser_pcm::lease(),
                    browser_pcm::basis(),
                    original.identity(),
                    reference.clone(),
                    format,
                    PlaybackStart {
                        first_frame: 48_000,
                        lead_seconds: 0.05,
                    },
                )
                .map_err(error)?;
            let (current, stop_result) = replacement.into_parts();
            fixture.old_receipt = Some(original.clone());
            fixture.receipt = Some(current.clone());
            stop_result.map_err(error)?;
            let source = fixture
                .source
                .as_mut()
                .ok_or_else(|| JsValue::from_str("local PCM absent"))?;
            source.cache.lease_bytes(&source.source).map_err(error)?;
            let first = source
                .buffer
                .take()
                .ok_or_else(|| JsValue::from_str("local PCM buffer absent"))?;
            fixture
                .playback
                .enqueue(
                    &current,
                    &browser_pcm::lease(),
                    source.manifest,
                    0,
                    48_000,
                    first,
                )
                .map_err(error)?;
            let mut tail = browser_pcm::local_pcm(1);
            tail.cache.lease_bytes(&tail.source).map_err(error)?;
            fixture
                .playback
                .enqueue(
                    &current,
                    &browser_pcm::lease(),
                    tail.manifest,
                    1,
                    48_000 + FRAMES,
                    tail.buffer
                        .take()
                        .ok_or_else(|| JsValue::from_str("tail buffer absent"))?,
                )
                .map_err(error)?;
            fixture
                .playback
                .close(&current, &browser_pcm::lease(), 2, 48_000 + FRAMES * 2)
                .map_err(error)?;
            fixture
                .playback
                .pump(&browser_pcm::lease())
                .map_err(error)?;
            let before = fixture.playback.snapshot();
            if before.state != PlaybackState::Scheduled
                || before.sources != 1
                || before.ended_callbacks != 1
                || before.queue.retained_buffers != 2
                || before.queue.state != QueueState::Draining
                || before.retained_sample_bytes == 0
            {
                return Err(error(before));
            }
            let refused = fixture
                .playback
                .cancel_media(&original, &browser_pcm::lease(), &delayed);
            if refused != Err(PlaybackError::Queue(QueueError::StaleReceipt)) {
                return Err(error(refused));
            }
            let after = fixture.playback.snapshot();
            if !unchanged_resources(before, after) {
                return Err(error((before, after)));
            }

            let mut foreign_owner = browser_pcm::queue();
            let foreign = foreign_owner
                .replace(
                    &browser_pcm::lease(),
                    browser_pcm::basis(),
                    current.identity(),
                    reference,
                    format,
                    48_000,
                )
                .map_err(error)?
                .receipt;
            foreign_owner.dispose();
            drop(foreign_owner);
            let refused = fixture
                .playback
                .cancel_media(&foreign, &browser_pcm::lease(), &delayed);
            if refused != Err(PlaybackError::Queue(QueueError::ForeignOwner)) {
                return Err(error(refused));
            }
            let after = fixture.playback.snapshot();
            if !unchanged_resources(before, after) {
                return Err(error((before, after)));
            }
            fixture.render(
                "Media stop refused stale/foreign receipts; current real PCM and tail unchanged",
            );
            Ok(())
        })
    }

    #[wasm_bindgen]
    pub fn fixture_assert_current_media_stop_retains_until_terminal() -> Result<(), JsValue> {
        FIXTURE.with(|cell| {
            let mut fixture = cell.borrow_mut();
            let fixture = fixture
                .as_mut()
                .ok_or_else(|| JsValue::from_str("fixture busy"))?;
            let before = fixture.playback.snapshot();
            if before.sources != 1
                || before.queue.retained_buffers != 2
                || before.queue.state != QueueState::Draining
            {
                return Err(JsValue::from_str(
                    "run receipt-refusal probe before current media stop",
                ));
            }
            let current = fixture
                .receipt
                .clone()
                .ok_or_else(|| JsValue::from_str("current receipt absent"))?;
            let stopped = media_stopped(current.identity())?;
            fixture
                .playback
                .cancel_media(&current, &browser_pcm::lease(), &stopped)
                .map_err(error)?;
            let after = fixture.playback.snapshot();
            if after.queue.state != QueueState::WaitingForStop
                || after.queue.retained_buffers != 1
                || !after.queue.dispatched
                || after.queue.sample_bytes * 2 != before.queue.sample_bytes
                || after.sources != 1
                || after.ended_callbacks != 1
                || after.retained_sample_bytes == 0
                || after.stopped_buffers != before.stopped_buffers
                || after.completed_buffers != before.completed_buffers
            {
                return Err(error((before, after)));
            }
            fixture.media_stop_before = Some(before);
            fixture.render(
                "Current media stop accepted; real source and PCM retained for actual ended",
            );
            Ok(())
        })
    }

    #[wasm_bindgen]
    pub fn fixture_assert_current_media_stop_settled() -> Result<(), JsValue> {
        FIXTURE.with(|cell| {
            let mut fixture = cell.borrow_mut();
            let fixture = fixture
                .as_mut()
                .ok_or_else(|| JsValue::from_str("fixture busy"))?;
            let before = fixture
                .media_stop_before
                .ok_or_else(|| JsValue::from_str("current media stop not observed"))?;
            // Only the adapter's owned real onended event permits pump to release
            // this source. Calling early refuses qualification without fake callbacks.
            fixture
                .playback
                .pump(&browser_pcm::lease())
                .map_err(error)?;
            let after = fixture.playback.snapshot();
            let expected_stopped = before
                .stopped_buffers
                .checked_add(1)
                .ok_or_else(|| JsValue::from_str("fixture stop counter exhausted"))?;
            if after.state != PlaybackState::Ready
                || after.queue.state != QueueState::Cancelled
                || after.queue.retained_buffers != 0
                || after.queue.dispatched
                || after.sources != 0
                || after.ended_callbacks != 0
                || after.pending_promises != 0
                || after.retained_sample_bytes != 0
                || after.stopped_buffers != expected_stopped
                || after.completed_buffers != before.completed_buffers
            {
                return Err(error((before, after)));
            }
            fixture.media_stop_before = None;
            fixture.render(
                "Actual ended settled current media stop; source, callback and PCM released",
            );
            Ok(())
        })
    }

    #[wasm_bindgen]
    pub fn fixture_restore_stop_fault() -> Result<(), JsValue> {
        FIXTURE.with(|cell| {
            let mut fixture = cell.borrow_mut();
            fixture
                .as_mut()
                .ok_or_else(|| JsValue::from_str("fixture busy"))?
                .restore_stop_fault()
        })
    }

    #[wasm_bindgen]
    pub fn fixture_assert_replacement_stop_failure_receipt_visible() -> Result<(), JsValue> {
        FIXTURE.with(|cell| {
            let mut fixture = cell.borrow_mut();
            let fixture = fixture.as_mut().ok_or_else(|| JsValue::from_str("fixture busy"))?;
            fixture.restore_stop_fault()?;
            let initial = fixture.playback.snapshot();
            if initial.state != PlaybackState::Ready || initial.sources != 0
                || initial.queue.retained_buffers != 0 || initial.pending_promises != 0 {
                return Err(JsValue::from_str("unlock and settle old sources before replacement fault probe"));
            }
            fixture.replacement_fault_before = None;
            fixture.prepare(1).map_err(error)?;
            fixture.admit().map_err(error)?;
            let original = fixture.receipt.clone().ok_or_else(|| JsValue::from_str("old receipt absent"))?;
            let before = fixture.playback.snapshot();
            if before.sources != 1 || before.ended_callbacks != 1
                || before.queue.state != QueueState::Draining || before.queue.retained_buffers != 1 {
                return Err(error(before));
            }
            let next_generation = fixture.generation.checked_add(1)
                .ok_or_else(|| JsValue::from_str("fixture generation exhausted"))?;
            let reference = fixture.source.as_ref()
                .ok_or_else(|| JsValue::from_str("local source absent"))?.reference.clone();
            let invalid = fixture.playback.replace(
                &browser_pcm::lease(), browser_pcm::basis(), browser_pcm::identity(next_generation),
                reference, df_audio::PcmFormat::new(1, 48_000).map_err(error)?,
                PlaybackStart { first_frame: 48_000, lead_seconds: -1.0 },
            );
            if !matches!(invalid, Err(PlaybackError::InvalidTimeline)) {
                return Err(error(invalid));
            }
            let unchanged = fixture.playback.snapshot();
            if !unchanged_resources(before, unchanged) { return Err(error((before, unchanged))); }

            fixture.stop_fault = Some(StopFault::install()?);
            // prepare stores accepted new receipt/source before reporting stop_result.
            // Always restore the saved descriptor before inspecting any result.
            let result = fixture.prepare(2);
            let observed = fixture.stop_fault.as_ref()
                .map(|fault| fault.observed.get());
            fixture.restore_stop_fault()?;
            let observed = observed.ok_or_else(|| JsValue::from_str("stop fault owner absent"))?;
            if result != Err(PlaybackError::BrowserOperation)
                || observed != StopFaultObservation::StoppedThenThrew {
                return Err(error((result, observed)));
            }
            let current = fixture.receipt.clone().ok_or_else(|| JsValue::from_str("accepted receipt hidden"))?;
            if current.identity() != browser_pcm::identity(next_generation)
                || fixture.old_receipt.as_ref().map(AudioReceipt::identity) != Some(original.identity()) {
                return Err(JsValue::from_str("accepted replacement ownership was not retained"));
            }
            let waiting = fixture.playback.snapshot();
            if waiting.queue.state != QueueState::WaitingForStop
                || waiting.sources != 1 || waiting.ended_callbacks != 1
                || waiting.queue.sample_bytes != before.queue.sample_bytes
                || waiting.retained_sample_bytes != before.retained_sample_bytes
                || waiting.stopped_buffers != before.stopped_buffers
                || waiting.completed_buffers != before.completed_buffers {
                return Err(error((before, waiting)));
            }
            let admission = fixture.admit();
            if admission != Err(PlaybackError::Queue(QueueError::WaitingForStop))
                || !fixture.source.as_ref().is_some_and(|source| source.buffer.is_some()) {
                return Err(error(admission));
            }
            let delayed = media_stopped(original.identity())?;
            let refused = fixture.playback.cancel_media(&original, &browser_pcm::lease(), &delayed);
            if refused != Err(PlaybackError::Queue(QueueError::StaleReceipt)) {
                return Err(error(refused));
            }
            let after = fixture.playback.snapshot();
            if !unchanged_resources(waiting, after) { return Err(error((waiting, after))); }
            fixture.replacement_fault_before = Some(before);
            fixture.render("Native stop ran then failed; descriptor restored, accepted receipt visible, exact old source retained");
            Ok(())
        })
    }

    #[wasm_bindgen]
    pub fn fixture_assert_replacement_stop_failure_recovered() -> Result<(), JsValue> {
        FIXTURE.with(|cell| {
            let mut fixture = cell.borrow_mut();
            let fixture = fixture.as_mut().ok_or_else(|| JsValue::from_str("fixture busy"))?;
            let before = fixture.replacement_fault_before
                .ok_or_else(|| JsValue::from_str("replacement fault not observed"))?;
            fixture.restore_stop_fault()?;
            // Owned actual old onended must retire its ticket before pending new
            // PCM can be admitted. An early pump returns WaitingForStop and keeps it.
            fixture.action("pump").map_err(error)?;
            let after = fixture.playback.snapshot();
            let expected_stopped = before.stopped_buffers.checked_add(1)
                .ok_or_else(|| JsValue::from_str("fixture stop counter exhausted"))?;
            if after.state != PlaybackState::Scheduled || after.queue.state != QueueState::Draining
                || after.sources != 1 || after.ended_callbacks != 1 || after.queue.retained_buffers != 1
                || after.queue.sample_bytes != FRAMES as usize * 2 * size_of::<f32>()
                || after.retained_sample_bytes != after.queue.sample_bytes * 2
                || after.stopped_buffers != expected_stopped || after.completed_buffers != before.completed_buffers
                || fixture.source.as_ref().is_none_or(|source| source.buffer.is_some())
                || fixture.stop_fault.is_some() {
                return Err(error((before, after)));
            }
            fixture.replacement_fault_before = None;
            fixture.render("Actual old ended released only old ownership; accepted new stereo source scheduled once");
            Ok(())
        })
    }

    #[wasm_bindgen]
    pub fn fixture_assert_drained_after_actual_end() -> Result<(), JsValue> {
        FIXTURE.with(|cell| {
            let mut fixture = cell.borrow_mut();
            let fixture = fixture
                .as_mut()
                .ok_or_else(|| JsValue::from_str("fixture busy"))?;
            fixture
                .playback
                .pump(&browser_pcm::lease())
                .map_err(error)?;
            let snapshot = fixture.playback.snapshot();
            if snapshot.queue.state != QueueState::Drained
                || snapshot.sources != 0
                || snapshot.ended_callbacks != 0
                || snapshot.retained_sample_bytes != 0
                || snapshot.completed_buffers == 0
            {
                return Err(error(snapshot));
            }
            fixture.render("Actual ended observed and finite queue drained");
            Ok(())
        })
    }

    #[wasm_bindgen]
    pub async fn fixture_wait_closed() -> Result<(), JsValue> {
        let mut fixture = take()?;
        if let Err(error) = fixture.restore_stop_fault() {
            restore(fixture);
            return Err(error);
        }
        let result = fixture.playback.wait_closed().await;
        fixture.render(&format!("Close settled: {result:?}"));
        let snapshot = fixture.playback.snapshot();
        let clean = snapshot.state == PlaybackState::Closed
            && snapshot.queue.state == QueueState::Disposed
            && snapshot.sources == 0
            && snapshot.ended_callbacks == 0
            && snapshot.pending_promises == 0
            && snapshot.retained_sample_bytes == 0;
        restore(fixture);
        result.map_err(error)?;
        if !clean {
            return Err(error(snapshot));
        }
        Ok(())
    }

    #[wasm_bindgen]
    pub fn fixture_unmount_closed() -> Result<(), JsValue> {
        let mut fixture = take()?;
        if let Err(error) = fixture.restore_stop_fault() {
            restore(fixture);
            return Err(error);
        }
        if fixture.playback.snapshot().state != PlaybackState::Closed {
            restore(fixture);
            return Err(JsValue::from_str("close and await before unmount"));
        }
        for button in &fixture.buttons {
            if let Err(error) = button.element.remove_event_listener_with_callback(
                "click",
                button.on_click.as_ref().unchecked_ref(),
            ) {
                restore(fixture);
                return Err(error);
            }
        }
        if let Some(parent) = fixture.root.parent_node()
            && let Err(error) = parent.remove_child(&fixture.root)
        {
            restore(fixture);
            return Err(error);
        }
        Ok(())
    }
}
