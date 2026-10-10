use df_providers::{
    BillingUnit, CandidateRouteId, Capability, Gate, GateStatus, RoundingRule,
    assess_dispatch_gates, candidate_for, candidate_routes,
};

#[test]
fn selected_routes_keep_dated_rates_license_and_endpoint_sources() {
    let routes = candidate_routes();
    let eleven = routes[0];
    assert_eq!(eleven.id(), CandidateRouteId::ElevenFlashV25Tts);
    assert_eq!(eleven.capability(), Capability::TextToSpeech);
    assert_eq!(eleven.supplier(), "ElevenLabs");
    assert_eq!(eleven.model(), "eleven_flash_v2_5");
    assert_eq!(
        eleven.endpoint(),
        "POST https://api.elevenlabs.io/v1/text-to-speech/:voice_id/stream"
    );
    assert_eq!(eleven.published_rate().amount_micros(), 40_000);
    assert_eq!(
        eleven.published_rate().unit(),
        BillingUnit::ThousandCharacters
    );
    assert_eq!(
        eleven.published_rate().rounding(),
        RoundingRule::NonePublished
    );
    assert_eq!(
        eleven.published_rate().evidence().observed_on(),
        "2026-10-10"
    );
    assert_eq!(
        eleven.published_rate().evidence().url(),
        "https://elevenlabs.io/pricing/api"
    );
    assert_eq!(
        eleven.route_evidence().url(),
        "https://elevenlabs.io/docs/api-reference/text-to-speech/stream"
    );
    assert_eq!(
        eleven.availability_evidence().url(),
        "https://elevenlabs.io/elevenapi-terms"
    );
    assert!(
        eleven
            .license()
            .summary()
            .contains("Paid-plan commercial use")
    );
    assert_eq!(
        eleven.license().evidence().url(),
        "https://elevenlabs.io/terms-of-use"
    );

    let fal = routes[1];
    assert_eq!(fal.id(), CandidateRouteId::FalFluxSchnellDisposableImage);
    assert_eq!(fal.capability(), Capability::DisposableImage);
    assert_eq!(fal.supplier(), "fal");
    assert_eq!(fal.model(), "fal-ai/flux/schnell");
    assert_eq!(fal.published_rate().amount_micros(), 3_000);
    assert_eq!(fal.published_rate().unit(), BillingUnit::Megapixel);
    assert_eq!(
        fal.published_rate().rounding(),
        RoundingRule::RoundUpToWholeMegapixels
    );
    assert_eq!(fal.published_rate().evidence().observed_on(), "2026-10-10");
    assert_eq!(
        fal.published_rate().evidence().url(),
        "https://fal.ai/models/fal-ai/flux/schnell"
    );
    assert_eq!(
        fal.route_evidence().url(),
        "https://fal.ai/models/fal-ai/flux/schnell/api"
    );
    assert_eq!(
        fal.availability_evidence().url(),
        "https://fal.ai/legal/api-services"
    );
    assert!(fal.license().summary().contains("labels commercial use"));
    assert_eq!(
        fal.license().evidence().url(),
        "https://fal.ai/legal/terms-of-service"
    );
}

#[test]
fn unselected_capabilities_are_explicitly_unsupported() {
    for capability in [
        Capability::Text,
        Capability::Stt,
        Capability::Image,
        Capability::Video,
        Capability::Sound,
    ] {
        let refusal = candidate_for(capability).expect_err("no route is selected");
        assert_eq!(refusal.capability(), capability);
    }
}

#[test]
fn candidate_routes_remain_blocked_until_every_gate_is_qualified() {
    for route in candidate_routes() {
        let block = route
            .dispatch_block()
            .expect_err("live qualification is absent");
        assert_eq!(block.gate(), Gate::AccountEntitlement);
        assert_eq!(block.status(), GateStatus::Unperformed);
    }

    assert!(assess_dispatch_gates([GateStatus::Qualified; 5]).is_ok());
    for status in [
        GateStatus::Pending,
        GateStatus::Failed,
        GateStatus::Unperformed,
        GateStatus::Unsupported,
    ] {
        let statuses = [
            GateStatus::Qualified,
            GateStatus::Qualified,
            GateStatus::Qualified,
            GateStatus::Qualified,
            status,
        ];
        let block = assess_dispatch_gates(statuses).expect_err("unqualified gate blocks");
        assert_eq!(block.gate(), Gate::RightsApproval);
        assert_eq!(block.status(), status);
    }
}

#[test]
fn candidate_debug_contains_no_credential_or_request_payload() {
    for route in candidate_routes() {
        let debug = format!("{route:?}");
        assert!(!debug.contains("api_key"));
        assert!(!debug.contains("secret"));
        assert!(!debug.contains("prompt payload"));
        assert!(debug.contains("dispatchable: false"));
    }
}
