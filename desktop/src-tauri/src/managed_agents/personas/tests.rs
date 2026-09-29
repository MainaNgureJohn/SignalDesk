use super::{
    built_in_persona_records, ensure_persona_ids_are_active, ensure_persona_is_active,
    merge_personas, migrate_retired_personas, validate_persona_activation_change,
    validate_persona_deletion, BUILT_IN_PERSONAS, FIZZ_MILESTONE_1_SYSTEM_PROMPT,
    FIZZ_SYSTEM_PROMPT, RETIRED_PERSONAS, SIGNALDESK_CONFIRMATION_GATE_ENV,
    SIGNALDESK_FIZZ_CODEX_CONFIG, SIGNALDESK_LEGACY_READ_ONLY_CODEX_CONFIG,
    SIGNALDESK_PRE_WALLET_FIZZ_CODEX_CONFIG, SIGNALDESK_PRE_WALLET_READ_ONLY_CODEX_CONFIG,
    SIGNALDESK_READ_ONLY_CODEX_CONFIG,
};
use crate::managed_agents::discovery::effective_agent_command;
use crate::managed_agents::AgentDefinition;

fn custom_persona(id: &str, display_name: &str) -> AgentDefinition {
    AgentDefinition {
        description: None,
        id: id.to_string(),
        display_name: display_name.to_string(),
        avatar_url: Some("https://example.com/avatar.png".to_string()),
        system_prompt: "Custom prompt".to_string(),
        runtime: None,
        model: None,
        provider: None,
        name_pool: Vec::new(),
        is_builtin: false,
        is_active: true,
        shared: false,
        source_team: None,
        source_team_persona_slug: None,
        catalog_source: None,
        team_catalog_source: None,
        env_vars: std::collections::BTreeMap::new(),
        respond_to: None,
        respond_to_allowlist: Vec::new(),
        parallelism: None,
        created_at: "2026-03-19T00:00:00Z".to_string(),
        updated_at: "2026-03-19T00:00:00Z".to_string(),
    }
}

#[test]
fn merge_personas_adds_missing_built_ins() {
    let (records, changed) = merge_personas(Vec::new(), "2026-03-19T00:00:00Z");

    assert!(changed);
    assert_eq!(records.len(), BUILT_IN_PERSONAS.len());
    assert!(records.iter().all(|record| record.is_builtin));
    assert!(records
        .iter()
        .all(|record| { record.runtime.as_deref() == Some("codex") }));
    let display_names: Vec<&str> = records
        .iter()
        .map(|record| record.display_name.as_str())
        .collect();
    assert_eq!(display_names, vec!["Fizz", "Honey", "Pollen"]);
    let active_ids: Vec<&str> = records
        .iter()
        .filter(|record| record.is_active)
        .map(|record| record.id.as_str())
        .collect();
    assert_eq!(
        active_ids,
        vec!["builtin:fizz", "builtin:honey", "builtin:bumble"]
    );

    let fizz = records
        .iter()
        .find(|record| record.id == "builtin:fizz")
        .unwrap();
    assert_eq!(
        fizz.env_vars.get("CODEX_CONFIG").map(String::as_str),
        Some(SIGNALDESK_FIZZ_CODEX_CONFIG)
    );
    assert_eq!(
        fizz.env_vars
            .get(SIGNALDESK_CONFIRMATION_GATE_ENV)
            .map(String::as_str),
        Some("1")
    );
    assert!(!SIGNALDESK_FIZZ_CODEX_CONFIG.contains("enabled_tools"));
    let fizz_config: serde_json::Value =
        serde_json::from_str(SIGNALDESK_FIZZ_CODEX_CONFIG).unwrap();
    assert_eq!(
        fizz_config["mcp_servers"]["signaldesk-web3"]["command"],
        "signaldesk-web3-mcp"
    );

    for persona_id in ["builtin:honey", "builtin:bumble"] {
        let persona = records
            .iter()
            .find(|record| record.id == persona_id)
            .unwrap();
        assert_eq!(
            persona.env_vars.get("CODEX_CONFIG").map(String::as_str),
            Some(SIGNALDESK_READ_ONLY_CODEX_CONFIG)
        );
        let config: serde_json::Value =
            serde_json::from_str(SIGNALDESK_READ_ONLY_CODEX_CONFIG).unwrap();
        assert_eq!(
            config["mcp_servers"]["signaldesk-web3"]["command"],
            "signaldesk-web3-mcp"
        );
        assert_eq!(
            config["mcp_servers"]["signaldesk-web3"]["enabled_tools"],
            serde_json::json!(["research_bsc_bstock", "preview_bsc_bstock_buy"])
        );
        assert!(!config["mcp_servers"]["signaldesk-web3"]["enabled_tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool.as_str() == Some("execute_agentic_wallet_bstock_buy")));
        assert!(!persona
            .env_vars
            .contains_key(SIGNALDESK_CONFIRMATION_GATE_ENV));
    }
}

#[test]
fn merge_personas_upgrades_pre_wallet_configs_without_overwriting_custom_settings() {
    let mut fizz = custom_persona("builtin:fizz", "Fizz");
    fizz.is_builtin = true;
    fizz.system_prompt = super::FIZZ_PRE_WALLET_SYSTEM_PROMPT.to_string();
    fizz.env_vars.insert(
        "CODEX_CONFIG".to_string(),
        SIGNALDESK_PRE_WALLET_FIZZ_CODEX_CONFIG.to_string(),
    );

    let mut honey = custom_persona("builtin:honey", "Honey");
    honey.is_builtin = true;
    honey.env_vars.insert(
        "CODEX_CONFIG".to_string(),
        SIGNALDESK_PRE_WALLET_READ_ONLY_CODEX_CONFIG.to_string(),
    );

    let mut custom_fizz = custom_persona("custom:fizz", "Custom Fizz");
    custom_fizz.env_vars.insert(
        "CODEX_CONFIG".to_string(),
        "{\"custom\":true}".to_string(),
    );

    let (records, changed) = merge_personas(
        vec![fizz, honey, custom_fizz],
        "2026-09-19T00:00:00Z",
    );

    assert!(changed);
    let fizz = records.iter().find(|record| record.id == "builtin:fizz").unwrap();
    assert_eq!(fizz.system_prompt, FIZZ_SYSTEM_PROMPT);
    assert_eq!(
        fizz.env_vars.get("CODEX_CONFIG").map(String::as_str),
        Some(SIGNALDESK_FIZZ_CODEX_CONFIG)
    );
    let honey = records.iter().find(|record| record.id == "builtin:honey").unwrap();
    assert_eq!(
        honey.env_vars.get("CODEX_CONFIG").map(String::as_str),
        Some(SIGNALDESK_READ_ONLY_CODEX_CONFIG)
    );
    let custom = records.iter().find(|record| record.id == "custom:fizz").unwrap();
    assert_eq!(
        custom.env_vars.get("CODEX_CONFIG").map(String::as_str),
        Some("{\"custom\":true}")
    );
}

#[test]
fn merge_personas_adds_web3_tool_to_exact_legacy_read_only_config() {
    let mut honey = custom_persona("builtin:honey", "Honey");
    honey.is_builtin = true;
    honey.env_vars.insert(
        "CODEX_CONFIG".to_string(),
        SIGNALDESK_LEGACY_READ_ONLY_CODEX_CONFIG.to_string(),
    );

    let (records, changed) = merge_personas(vec![honey], "2026-09-18T00:00:00Z");

    assert!(changed);
    let honey = records
        .iter()
        .find(|record| record.id == "builtin:honey")
        .unwrap();
    assert_eq!(
        honey.env_vars.get("CODEX_CONFIG").map(String::as_str),
        Some(SIGNALDESK_READ_ONLY_CODEX_CONFIG)
    );
}

#[test]
fn merge_personas_preserves_custom_records() {
    let custom = custom_persona("custom:test", "Custom");
    let (records, changed) = merge_personas(vec![custom.clone()], "2026-03-19T00:00:00Z");

    assert!(changed);
    assert!(records.iter().any(|record| record.id == custom.id));
}

#[test]
fn merge_personas_preserves_builtin_edits() {
    let mut edited_builtin = custom_persona("builtin:fizz", "My Fizz");
    edited_builtin.is_builtin = true;
    edited_builtin.is_active = true;
    edited_builtin.system_prompt = "User-edited instructions".to_string();
    edited_builtin.name_pool = vec!["User-edited name".to_string()];
    edited_builtin.env_vars =
        std::collections::BTreeMap::from([("USER_SETTING".to_string(), "value".to_string())]);

    let (records, changed) = merge_personas(vec![edited_builtin.clone()], "2026-03-19T00:00:00Z");

    assert!(changed); // The remaining seeded built-ins are added.
    let fizz = records
        .iter()
        .find(|record| record.id == "builtin:fizz")
        .expect("fizz built-in should exist");
    assert_eq!(fizz.display_name, edited_builtin.display_name);
    assert_eq!(fizz.system_prompt, edited_builtin.system_prompt);
    assert_eq!(fizz.name_pool, edited_builtin.name_pool);
    assert_eq!(fizz.env_vars, edited_builtin.env_vars);
    assert_eq!(fizz.is_active, edited_builtin.is_active);
}

#[test]
fn merge_personas_repairs_signaldesk_codex_command_used_as_runtime_id() {
    let mut stale_builtin = custom_persona("builtin:fizz", "Fizz");
    stale_builtin.is_builtin = true;
    stale_builtin.runtime = Some("codex-acp".to_string());

    let (records, changed) = merge_personas(vec![stale_builtin], "2026-09-07T00:00:00Z");

    assert!(changed);
    let fizz = records
        .iter()
        .find(|record| record.id == "builtin:fizz")
        .expect("fizz built-in should exist");
    assert_eq!(fizz.runtime.as_deref(), Some("codex"));
}

#[test]
fn merge_personas_upgrades_exact_read_only_fizz_to_confirmed_actions() {
    let mut stale_builtin = custom_persona("builtin:fizz", "Fizz");
    stale_builtin.is_builtin = true;
    stale_builtin.runtime = Some("codex".to_string());
    stale_builtin.system_prompt = FIZZ_MILESTONE_1_SYSTEM_PROMPT.to_string();
    stale_builtin.env_vars.insert(
        "CODEX_CONFIG".to_string(),
        SIGNALDESK_READ_ONLY_CODEX_CONFIG.to_string(),
    );

    let (records, changed) = merge_personas(vec![stale_builtin], "2026-09-08T00:00:00Z");

    assert!(changed);
    let fizz = records
        .iter()
        .find(|record| record.id == "builtin:fizz")
        .expect("fizz built-in should exist");
    assert_eq!(fizz.system_prompt, FIZZ_SYSTEM_PROMPT);
    assert_eq!(
        fizz.env_vars.get("CODEX_CONFIG").map(String::as_str),
        Some(SIGNALDESK_FIZZ_CODEX_CONFIG)
    );
    assert_eq!(
        fizz.env_vars
            .get(SIGNALDESK_CONFIRMATION_GATE_ENV)
            .map(String::as_str),
        Some("1")
    );
    assert_eq!(fizz.updated_at, "2026-09-08T00:00:00Z");
}

#[test]
fn merge_personas_restores_builtin_marker_without_resetting_edits() {
    let mut edited_builtin = custom_persona("builtin:fizz", "My Fizz");
    edited_builtin.is_builtin = false;

    let (records, changed) = merge_personas(vec![edited_builtin], "2026-03-19T00:00:00Z");

    assert!(changed);
    let fizz = records
        .iter()
        .find(|record| record.id == "builtin:fizz")
        .expect("fizz built-in should exist");
    assert!(fizz.is_builtin);
    assert_eq!(fizz.display_name, "My Fizz");
}

#[test]
fn merge_personas_adds_fizz_and_retires_old_builtins_for_existing_store() {
    let mut legacy_builtins = vec![custom_persona("builtin:solo", "Solo")];
    for persona in &mut legacy_builtins {
        persona.is_builtin = true;
        persona.avatar_url = None;
    }

    let (records, changed) = merge_personas(legacy_builtins, "2026-03-19T00:00:00Z");

    assert!(changed);
    let fizz = records
        .iter()
        .find(|record| record.id == "builtin:fizz")
        .expect("fizz built-in should exist");
    assert!(fizz.is_builtin);
    assert!(fizz.is_active);

    let solo = records
        .iter()
        .find(|record| record.id == "builtin:solo")
        .expect("old solo record should be retained as retired custom persona");
    assert!(!solo.is_builtin);
    assert!(!solo.is_active);
    assert_eq!(solo.display_name, "Solo (retired)");
}

#[test]
fn merge_personas_demotes_retired_builtins() {
    // custom_persona uses "Custom prompt", which doesn't match the original
    // retired system prompt, so the migration pass soft-deprecates rather
    // than removes the record.
    let mut retired = custom_persona("builtin:reviewer", "Reviewer");
    retired.is_builtin = true;
    retired.is_active = true;
    let original_created_at = retired.created_at.clone();

    let (records, changed) = merge_personas(vec![retired], "2026-04-01T00:00:00Z");

    assert!(changed);
    let demoted = records
        .iter()
        .find(|record| record.id == "builtin:reviewer")
        .expect("retired built-in should be retained as a soft-deprecated custom persona");
    assert!(!demoted.is_builtin);
    // migrate_retired_personas deactivates customized retired personas.
    assert!(!demoted.is_active);
    assert_eq!(demoted.display_name, "Reviewer (retired)");
    assert_eq!(demoted.created_at, original_created_at);
    assert_eq!(demoted.updated_at, "2026-04-01T00:00:00Z");
}

#[test]
fn ensure_persona_is_active_rejects_missing_personas() {
    let err = ensure_persona_is_active(&[], "missing").unwrap_err();

    assert_eq!(err, "agent missing not found");
}

#[test]
fn ensure_persona_is_active_rejects_inactive_personas() {
    let mut persona = custom_persona("builtin:fizz", "Fizz");
    persona.is_builtin = true;
    persona.is_active = false;

    let err = ensure_persona_is_active(&[persona], "builtin:fizz").unwrap_err();

    assert_eq!(err, "Fizz is not in My Agents.");
}

#[test]
fn ensure_persona_ids_are_active_checks_each_requested_id() {
    let personas = vec![
        custom_persona("custom:alpha", "Alpha"),
        custom_persona("custom:beta", "Beta"),
    ];

    assert!(ensure_persona_ids_are_active(
        &personas,
        &["custom:alpha".to_string(), "custom:beta".to_string()],
    )
    .is_ok());
}

#[test]
fn validate_persona_activation_change_rejects_non_builtins() {
    let persona = custom_persona("custom:alpha", "Alpha");

    let err = validate_persona_activation_change(&persona, false, false, false).unwrap_err();

    assert_eq!(
        err,
        "Only built-in agents can be added to or removed from My Agents."
    );
}

#[test]
fn validate_persona_activation_change_rejects_managed_agent_references() {
    let mut persona = custom_persona("builtin:fizz", "Fizz");
    persona.is_builtin = true;

    let err = validate_persona_activation_change(&persona, false, true, false).unwrap_err();

    assert_eq!(
        err,
        "Fizz is still assigned to a managed agent. Remove or reassign those agents first."
    );
}

#[test]
fn validate_persona_activation_change_rejects_team_references() {
    let mut persona = custom_persona("builtin:fizz", "Fizz");
    persona.is_builtin = true;

    let err = validate_persona_activation_change(&persona, false, false, true).unwrap_err();

    assert_eq!(
        err,
        "Fizz is still referenced by a team. Remove it from those teams first."
    );
}

#[test]
fn validate_persona_activation_change_allows_safe_builtin_updates() {
    let mut persona = custom_persona("builtin:fizz", "Fizz");
    persona.is_builtin = true;

    assert!(validate_persona_activation_change(&persona, true, false, false).is_ok());
    assert!(validate_persona_activation_change(&persona, false, false, false).is_ok());
}

#[test]
fn validate_persona_deletion_rejects_builtins() {
    let mut persona = custom_persona("builtin:fizz", "Fizz");
    persona.is_builtin = true;

    let err = validate_persona_deletion(&persona, false).unwrap_err();

    assert_eq!(err, "Built-in agents cannot be deleted.");
}

#[test]
fn validate_persona_deletion_rejects_team_references() {
    let persona = custom_persona("custom:alpha", "Alpha");

    let err = validate_persona_deletion(&persona, true).unwrap_err();

    assert_eq!(
        err,
        "Alpha is still referenced by a team. Remove it from those teams first."
    );
}

#[test]
fn validate_persona_deletion_allows_safe_custom_personas() {
    let persona = custom_persona("custom:alpha", "Alpha");

    assert!(validate_persona_deletion(&persona, false).is_ok());
}

// ── migrate_retired_personas ──────────────────────────────────────────────────

#[test]
fn migrate_retires_unmodified_personas() {
    let now = "2026-04-01T00:00:00Z";
    // Simulate a store from before the Fizz transition: all 6
    // retired personas with original system prompts.
    let mut stored: Vec<AgentDefinition> = RETIRED_PERSONAS
        .iter()
        .map(|(id, prompt)| AgentDefinition {
            id: id.to_string(),
            system_prompt: prompt.to_string(),
            is_builtin: false, // already demoted by merge_personas
            ..custom_persona(id, "Test Persona")
        })
        .collect();

    let changed = migrate_retired_personas(&mut stored, now);

    assert!(changed);
    assert_eq!(
        stored.len(),
        RETIRED_PERSONAS.len(),
        "all retired personas should be soft-deprecated, not removed",
    );
    assert!(
        stored
            .iter()
            .all(|r| r.display_name.ends_with(" (retired)")),
        "all retired personas should have ' (retired)' suffix",
    );
    assert!(
        stored.iter().all(|r| !r.is_active),
        "all retired personas should be inactive",
    );
    assert!(
        stored.iter().all(|r| r.updated_at == now),
        "all retired personas should have refreshed updated_at",
    );
}

#[test]
fn migrate_preserves_customized_personas() {
    let now = "2026-04-01T00:00:00Z";
    let mut stored = vec![AgentDefinition {
        id: "builtin:researcher".to_string(),
        display_name: "My Researcher".to_string(),
        system_prompt: "My custom research workflow with special instructions".to_string(),
        is_builtin: false,
        is_active: true,
        shared: false,
        ..custom_persona("builtin:researcher", "My Researcher")
    }];

    let changed = migrate_retired_personas(&mut stored, now);

    assert!(changed);
    assert_eq!(stored.len(), 1);
    let record = &stored[0];
    assert_eq!(record.display_name, "My Researcher (retired)");
    assert!(!record.is_active);
    assert_eq!(
        record.system_prompt,
        "My custom research workflow with special instructions"
    );
    assert_eq!(record.updated_at, now);
}

#[test]
fn migrate_is_idempotent() {
    let now = "2026-04-01T00:00:00Z";

    // 1. Non-retired persona — no-op.
    let mut stored = vec![custom_persona("custom:test", "Custom")];
    assert!(!migrate_retired_personas(&mut stored, now));
    assert_eq!(stored.len(), 1);

    // 2. Already-retired persona (display_name ends with " (retired)") — no-op.
    let mut stored_with_retired = vec![AgentDefinition {
        id: "builtin:researcher".to_string(),
        display_name: "Researcher (retired)".to_string(),
        system_prompt: "My custom prompt".to_string(),
        is_builtin: false,
        is_active: false,
        shared: false,
        ..custom_persona("builtin:researcher", "Researcher (retired)")
    }];
    assert!(
        !migrate_retired_personas(&mut stored_with_retired, now),
        "already-retired persona should not trigger another change"
    );

    // 3. Retired persona still marked is_builtin: true (pre-demotion).
    // migrate_retired_personas should still soft-deprecate it.
    let mut stored_pre_demotion = vec![AgentDefinition {
        id: "builtin:reviewer".to_string(),
        display_name: "Reviewer".to_string(),
        system_prompt: "Custom review prompt".to_string(),
        is_builtin: true,
        is_active: true,
        shared: false,
        ..custom_persona("builtin:reviewer", "Reviewer")
    }];
    assert!(migrate_retired_personas(&mut stored_pre_demotion, now));
    assert_eq!(stored_pre_demotion[0].display_name, "Reviewer (retired)");
    assert!(!stored_pre_demotion[0].is_active);

    // 4. Run again on result of (3) — should be no-op.
    assert!(!migrate_retired_personas(&mut stored_pre_demotion, now));
}

// ── SignalDesk Codex harness ──────────────────────────────────────────────────

#[test]
fn signaldesk_builtins_pin_codex_acp() {
    let records = built_in_persona_records("2026-01-01T00:00:00Z");
    for persona_id in ["builtin:fizz", "builtin:honey", "builtin:bumble"] {
        assert_eq!(
            effective_agent_command(Some(persona_id), &records, None),
            "codex-acp",
            "SignalDesk built-ins must use Codex so Binance MCP is available"
        );
    }
}
