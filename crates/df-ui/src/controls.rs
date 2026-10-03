use std::{
    cell::{Cell, RefCell},
    fmt,
    rc::Rc,
};

use wasm_bindgen::{JsCast, closure::Closure};
use web_sys::{Document, Element, Event, EventTarget, HtmlButtonElement, HtmlInputElement};

use crate::draft_state::DraftState;
use crate::{DraftError, MAX_DRAFT_UTF16_UNITS, UiError, action_button, text_input};

/// A control identifier or DOM operation failed; private draft text is never included.
#[derive(Debug)]
pub enum ControlError {
    Dom(UiError),
    InvalidId,
    IdAlreadyMounted,
    Draft(DraftError),
    CallbackAlreadyRegistered,
}

impl fmt::Display for ControlError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Dom(error) => fmt::Display::fmt(error, formatter),
            Self::InvalidId => formatter.write_str("control identifier is invalid"),
            Self::IdAlreadyMounted => formatter.write_str("control identifier is already mounted"),
            Self::Draft(error) => fmt::Display::fmt(error, formatter),
            Self::CallbackAlreadyRegistered => {
                formatter.write_str("control callback is already registered")
            }
        }
    }
}

impl std::error::Error for ControlError {}

impl From<UiError> for ControlError {
    fn from(error: UiError) -> Self {
        Self::Dom(error)
    }
}

impl From<DraftError> for ControlError {
    fn from(error: DraftError) -> Self {
        Self::Draft(error)
    }
}

type DraftCallback = Box<dyn FnMut(Result<String, DraftError>)>;
type ActionCallback = Box<dyn FnMut()>;

struct OwnedListener {
    target: EventTarget,
    name: &'static str,
    active: Rc<Cell<bool>>,
    callback: Option<Closure<dyn FnMut(Event)>>,
}

impl OwnedListener {
    fn bind(
        target: EventTarget,
        name: &'static str,
        mut operation: impl FnMut(Event) + 'static,
    ) -> Result<Self, ControlError> {
        let active = Rc::new(Cell::new(true));
        let callback_active = Rc::clone(&active);
        let callback = Closure::wrap(Box::new(move |event| {
            if callback_active.get() {
                operation(event);
            }
        }) as Box<dyn FnMut(Event)>);
        target.add_event_listener_with_callback(name, callback.as_ref().unchecked_ref())?;
        Ok(Self {
            target,
            name,
            active,
            callback: Some(callback),
        })
    }

    fn dispose(&mut self) -> Result<(), ControlError> {
        self.active.set(false);
        if let Some(callback) = self.callback.as_ref() {
            self.target.remove_event_listener_with_callback(
                self.name,
                callback.as_ref().unchecked_ref(),
            )?;
        }
        self.callback = None;
        Ok(())
    }
}

impl Drop for OwnedListener {
    fn drop(&mut self) {
        // Explicit widget disposal returns DOM failures. Drop still fences callbacks.
        // If removal fails, keep only the inert closure alive rather than leave a
        // browser callback pointing at freed Rust code. Widget Drop clears draft and
        // user callbacks before this fallback can retain anything.
        if self.dispose().is_err()
            && let Some(callback) = self.callback.take()
        {
            callback.forget();
        }
    }
}

impl From<wasm_bindgen::JsValue> for ControlError {
    fn from(error: wasm_bindgen::JsValue) -> Self {
        Self::Dom(UiError::Browser(error))
    }
}

/// Already localized plain text supplied by the presentation owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InputFeedback<'a> {
    None,
    Pending(&'a str),
    Rejected(&'a str),
}

impl<'a> InputFeedback<'a> {
    fn message(self) -> Option<&'a str> {
        match self {
            Self::None => None,
            Self::Pending(message) | Self::Rejected(message) => Some(message),
        }
    }
}

/// Only the current presentation owner can decide whether a draft is still valid.
/// Preserve is used for same-owner updates and rejected submissions. Replace clears
/// obsolete member/run/offer text, or deliberately applies a new controlled value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DraftUpdate<'a> {
    Preserve,
    Replace(&'a str),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TextInputView<'a> {
    pub label: &'a str,
    pub enabled: bool,
    pub feedback: InputFeedback<'a>,
}

/// A stable native text field with at most one owned input listener and no remote work.
/// The caller reconciles this handle rather than remounting it.
/// Identifier must be unique among mounted fields and detached sibling creations.
pub struct ControlledTextInput {
    root: Element,
    caption: Element,
    input: HtmlInputElement,
    feedback: Element,
    feedback_id: String,
    state: Rc<RefCell<DraftState>>,
    change_callback: Rc<RefCell<Option<DraftCallback>>>,
    callback_registered: Cell<bool>,
    listener: RefCell<Option<OwnedListener>>,
}

impl ControlledTextInput {
    /// Identifier contains 1..=128 ASCII letters, digits, hyphens, or underscores.
    /// Labels and feedback must contain non-whitespace text. All content is literal.
    pub fn create(
        document: &Document,
        identifier: &str,
        view: TextInputView<'_>,
        initial_draft: &str,
    ) -> Result<Self, ControlError> {
        validate_id(identifier)?;
        validate_view(view)?;
        let state = DraftState::create(initial_draft)?;
        let feedback_id = format!("{identifier}-feedback");
        if document.get_element_by_id(&feedback_id).is_some() {
            return Err(ControlError::IdAlreadyMounted);
        }
        let root = document.create_element("div")?;
        root.set_class_name("df-ui-field");
        let (label, input) = text_input(document, view.label)?;
        input.set_max_length(MAX_DRAFT_UTF16_UNITS as i32);
        let caption = label
            .first_element_child()
            .ok_or(UiError::WrongElementType)?;
        let feedback = document.create_element("p")?;
        feedback.set_id(&feedback_id);
        feedback.set_class_name("df-ui-feedback");
        feedback.set_attribute("role", "status")?;
        feedback.set_attribute("aria-live", "polite")?;
        feedback.set_attribute("aria-atomic", "true")?;
        root.append_child(&label)?;
        root.append_child(&feedback)?;
        let field = Self {
            root,
            caption,
            input,
            feedback,
            feedback_id,
            state: Rc::new(RefCell::new(state)),
            change_callback: Rc::new(RefCell::new(None)),
            callback_registered: Cell::new(false),
            listener: RefCell::new(None),
        };
        field.update(view, DraftUpdate::Replace(initial_draft))?;
        *field.listener.borrow_mut() = Some(field.bind_input_listener()?);
        Ok(field)
    }

    pub fn root(&self) -> &Element {
        &self.root
    }

    /// Native input is exposed for focus/selection; changes use the owned callback.
    pub fn input(&self) -> &HtmlInputElement {
        &self.input
    }

    pub fn draft(&self) -> String {
        self.state.borrow().value().to_owned()
    }

    /// Registers the single owned native input callback. Changed drafts are bounded;
    /// unavailable/overlong synthetic inputs restore the last controlled value and
    /// report a typed rejection. Same-value events do not repeat change callbacks.
    /// Callback must not synchronously dispatch another input event on this field.
    pub fn on_change(
        &self,
        callback: impl FnMut(Result<String, DraftError>) + 'static,
    ) -> Result<(), ControlError> {
        if self.state.borrow().is_disposed() {
            return Err(DraftError::Disposed.into());
        }
        if self.callback_registered.get() {
            return Err(ControlError::CallbackAlreadyRegistered);
        }
        *self.change_callback.borrow_mut() = Some(Box::new(callback));
        self.callback_registered.set(true);
        Ok(())
    }

    fn bind_input_listener(&self) -> Result<OwnedListener, ControlError> {
        let input = self.input.clone();
        let state = Rc::clone(&self.state);
        let owner_callback = Rc::clone(&self.change_callback);
        OwnedListener::bind(self.input.clone().into(), "input", move |_| {
            let proposed = input.value();
            let result = state.borrow_mut().change(&proposed);
            let event = match result {
                Ok(true) => Some(Ok(proposed)),
                Ok(false) => None,
                Err(error) => {
                    input.set_value(state.borrow().value());
                    Some(Err(error))
                }
            };
            if let Some(event) = event {
                let callback = owner_callback.borrow_mut().take();
                if let Some(mut callback) = callback {
                    callback(event);
                    if !state.borrow().is_disposed() {
                        *owner_callback.borrow_mut() = Some(callback);
                    }
                }
            }
        })
    }

    /// Changes only existing nodes. Preserve never assigns the input value, keeping
    /// local edits, selection, composition and focus intact. Pending/unavailable
    /// fields are read-only instead of disabled so their focused draft stays readable.
    /// This is presentation; submission still requires current server authorization.
    pub fn update(
        &self,
        view: TextInputView<'_>,
        draft: DraftUpdate<'_>,
    ) -> Result<(), ControlError> {
        validate_view(view)?;
        let pending = matches!(view.feedback, InputFeedback::Pending(_));
        let rejected = matches!(view.feedback, InputFeedback::Rejected(_));
        let replacement = match draft {
            DraftUpdate::Preserve => None,
            DraftUpdate::Replace(value) => Some(value),
        };
        self.state
            .borrow_mut()
            .reconcile(view.enabled, pending, replacement)?;
        self.caption.set_text_content(Some(view.label));
        self.input.set_read_only(!view.enabled || pending);
        self.input
            .set_attribute("aria-disabled", boolean(!view.enabled))?;
        self.input
            .set_attribute("aria-invalid", boolean(rejected))?;
        self.input.set_attribute("aria-busy", boolean(pending))?;
        match view.feedback.message() {
            Some(message) => {
                self.input
                    .set_attribute("aria-describedby", &self.feedback_id)?;
                self.feedback.set_text_content(Some(message));
            }
            None => {
                self.input.remove_attribute("aria-describedby")?;
                self.feedback.set_text_content(None);
            }
        }
        if let DraftUpdate::Replace(value) = draft
            && self.input.value() != value
        {
            self.input.set_value(value);
        }
        Ok(())
    }

    /// Removes only the DOM root for temporary reparenting. The handle and its one
    /// listener remain owned; permanent teardown uses dispose instead.
    pub fn unmount(&self) -> Result<(), ControlError> {
        if let Some(parent) = self.root.parent_node() {
            parent.remove_child(&self.root)?;
        }
        Ok(())
    }

    /// Terminal and idempotent: clears private draft/callback, fences input delivery,
    /// removes the owned listener and root. Call explicitly to observe DOM failures.
    pub fn dispose(&self) -> Result<(), ControlError> {
        self.state.borrow_mut().dispose();
        self.change_callback.borrow_mut().take();
        self.input.set_value("");
        self.input.set_read_only(true);
        if let Some(listener) = self.listener.borrow_mut().as_mut() {
            listener.dispose()?;
        }
        *self.listener.borrow_mut() = None;
        self.unmount()
    }
}

impl Drop for ControlledTextInput {
    fn drop(&mut self) {
        self.state.borrow_mut().dispose();
        self.change_callback.borrow_mut().take();
        self.input.set_value("");
        self.input.set_read_only(true);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ActionView<'a> {
    pub label: &'a str,
    pub enabled: bool,
    pub pending: bool,
}

/// Stable native button retaining I01 keyboard, focus and 44px target conventions.
/// The handle owns one listener; its callback sends an advertised typed command only.
pub struct ControlledAction {
    button: HtmlButtonElement,
    active: Rc<Cell<bool>>,
    action_callback: Rc<RefCell<Option<ActionCallback>>>,
    callback_registered: Cell<bool>,
    listener: RefCell<Option<OwnedListener>>,
}

impl ControlledAction {
    pub fn create(document: &Document, view: ActionView<'_>) -> Result<Self, ControlError> {
        let button = action_button(document, view.label, view.enabled && !view.pending)?;
        let action = Self {
            button,
            active: Rc::new(Cell::new(true)),
            action_callback: Rc::new(RefCell::new(None)),
            callback_registered: Cell::new(false),
            listener: RefCell::new(None),
        };
        action.update(view)?;
        *action.listener.borrow_mut() = Some(action.bind_action_listener()?);
        Ok(action)
    }

    pub fn element(&self) -> &HtmlButtonElement {
        &self.button
    }

    pub fn update(&self, view: ActionView<'_>) -> Result<(), ControlError> {
        if !self.active.get() {
            return Err(DraftError::Disposed.into());
        }
        validate_text(view.label)?;
        self.button.set_text_content(Some(view.label));
        self.button.set_disabled(!view.enabled || view.pending);
        self.button
            .set_attribute("aria-busy", boolean(view.pending))?;
        Ok(())
    }

    /// Native click covers keyboard Space/Enter and pointer activation; no duplicate
    /// custom key listener is installed. Synthetic clicks are also gated by disabled.
    pub fn on_activate(&self, callback: impl FnMut() + 'static) -> Result<(), ControlError> {
        if !self.active.get() {
            return Err(DraftError::Disposed.into());
        }
        if self.callback_registered.get() {
            return Err(ControlError::CallbackAlreadyRegistered);
        }
        *self.action_callback.borrow_mut() = Some(Box::new(callback));
        self.callback_registered.set(true);
        Ok(())
    }

    fn bind_action_listener(&self) -> Result<OwnedListener, ControlError> {
        let button = self.button.clone();
        let active = Rc::clone(&self.active);
        let owner_callback = Rc::clone(&self.action_callback);
        OwnedListener::bind(self.button.clone().into(), "click", move |_| {
            if active.get() && !button.disabled() {
                let callback = owner_callback.borrow_mut().take();
                if let Some(mut callback) = callback {
                    callback();
                    if active.get() {
                        *owner_callback.borrow_mut() = Some(callback);
                    }
                }
            }
        })
    }

    pub fn dispose(&self) -> Result<(), ControlError> {
        self.active.set(false);
        self.action_callback.borrow_mut().take();
        self.button.set_disabled(true);
        if let Some(listener) = self.listener.borrow_mut().as_mut() {
            listener.dispose()?;
        }
        *self.listener.borrow_mut() = None;
        if let Some(parent) = self.button.parent_node() {
            parent.remove_child(&self.button)?;
        }
        Ok(())
    }
}

impl Drop for ControlledAction {
    fn drop(&mut self) {
        self.active.set(false);
        self.action_callback.borrow_mut().take();
        self.button.set_disabled(true);
    }
}

fn validate_view(view: TextInputView<'_>) -> Result<(), ControlError> {
    validate_text(view.label)?;
    if let Some(message) = view.feedback.message() {
        validate_text(message)?;
    }
    Ok(())
}

fn validate_text(value: &str) -> Result<(), ControlError> {
    if value.trim().is_empty() {
        return Err(UiError::EmptyLabel.into());
    }
    Ok(())
}

fn validate_id(identifier: &str) -> Result<(), ControlError> {
    if identifier.is_empty()
        || identifier.len() > 128
        || !identifier
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
    {
        return Err(ControlError::InvalidId);
    }
    Ok(())
}

const fn boolean(value: bool) -> &'static str {
    if value { "true" } else { "false" }
}
