#[cfg(target_arch = "wasm32")]
#[path = "../support/browser_pcm.rs"]
mod browser_pcm;

#[cfg(target_arch = "wasm32")]
mod browser {
    use std::cell::RefCell;

    use df_audio::browser_playback::{
        BrowserPlayback, PlaybackError, PlaybackLimits, PlaybackStart, PlaybackState,
    };
    use df_audio::{AudioReceipt, QueueError, QueueState};
    use wasm_bindgen::{JsCast, JsValue, closure::Closure, prelude::wasm_bindgen};
    use web_sys::{Element, Event};

    use super::browser_pcm::{self, FRAMES, LocalPcm};

    struct ButtonListener {
        element: Element,
        on_click: Closure<dyn FnMut(Event)>,
    }

    struct Fixture {
        playback: BrowserPlayback,
        source: Option<LocalPcm>,
        receipt: Option<AudioReceipt>,
        old_receipt: Option<AudioReceipt>,
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

    impl Fixture {
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
            let receipt = self.playback.replace(
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
            self.old_receipt = self.receipt.take();
            self.receipt = Some(receipt);
            self.source = Some(source);
            Ok(())
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
        let fixture = take()?;
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
