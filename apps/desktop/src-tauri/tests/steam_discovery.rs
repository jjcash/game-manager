// SPDX-License-Identifier: GPL-3.0-or-later
// Offline contract tests only. Never locate/open Steam or write an application DB.
#[path = "support/steam_contract.rs"]
mod contract;
use contract::*;
use serde::Deserialize;
use std::{
    cell::Cell,
    collections::BTreeMap,
    io::{self, Read},
};

const INSTALLED: &[u8] = include_bytes!("fixtures/steam/installed.acf");
const NO_NAME: &[u8] = include_bytes!("fixtures/steam/no-cached-name.acf");
const INCOMPLETE: &[u8] = include_bytes!("fixtures/steam/incomplete.acf");
const TRUNCATED: &[u8] = include_bytes!("fixtures/steam/truncated.acf");

#[test]
fn installed_manifest_retains_unicode_and_separates_installation_from_ownership() {
    let row = manifest("appmanifest_410001.acf", INSTALLED).unwrap();
    assert_eq!(row.app_id, 410001);
    assert_eq!(row.title, "星の Café — Synthetic %_ \"Adventure\" 🎮");
    assert_eq!(row.install_dir, "Synthetic Adventure");
    assert_eq!(row.installation, Installation::ClientReportedPresent);
    assert_eq!(row.visibility, Visibility::ManifestRecord);
    assert_eq!(row.ownership, Ownership::Unknown);
    assert!(row.potentially_stale);
    assert!(!row.fallback_title);
    // Neither a valid nor fabricated LastOwner grants ownership or account scope.
    let changed = String::from_utf8(INSTALLED.to_vec()).unwrap().replace(
        "\"LastOwner\" \"0\"",
        "\"LastOwner\" \"fictional-previous-user\"",
    );
    assert_eq!(
        manifest("appmanifest_410001.acf", changed.as_bytes())
            .unwrap()
            .ownership,
        Ownership::Unknown
    );
}
#[test]
fn metadata_cache_is_optional_and_partial_installation_is_not_absence() {
    let unnamed = manifest("appmanifest_410003.acf", NO_NAME).unwrap();
    assert_eq!(unnamed.title, "Steam app 410003");
    assert!(unnamed.fallback_title);
    assert_eq!(unnamed.installation, Installation::ClientReportedPresent);
    let partial = manifest("appmanifest_410004.acf", INCOMPLETE).unwrap();
    assert_eq!(partial.installation, Installation::Unknown);
    assert_eq!(partial.ownership, Ownership::Unknown);
}
#[test]
fn manifest_flags_are_conservative_and_do_not_execute_content() {
    let source = String::from_utf8(INSTALLED.to_vec()).unwrap();
    for flag in [4, 6, 68, 70] {
        let changed = source.replace(
            "\"StateFlags\" \"4\"",
            &format!("\"StateFlags\" \"{flag}\""),
        );
        assert_eq!(
            manifest("appmanifest_410001.acf", changed.as_bytes())
                .unwrap()
                .installation,
            Installation::ClientReportedPresent
        );
    }
    for flag in [
        "0",
        "1",
        "32",
        "128",
        "2048",
        "260",
        "1048580",
        "9223372036854775808",
        "invalid",
        "-1",
    ] {
        let changed = source.replace(
            "\"StateFlags\" \"4\"",
            &format!("\"StateFlags\" \"{flag}\""),
        );
        assert_eq!(
            manifest("appmanifest_410001.acf", changed.as_bytes())
                .unwrap()
                .installation,
            Installation::Unknown
        );
    }
    let long_name = source.replace("Café", &"x".repeat(201));
    assert!(
        manifest("appmanifest_410001.acf", long_name.as_bytes())
            .unwrap()
            .fallback_title
    );
}
#[test]
fn parser_rejects_malformed_truncated_duplicate_and_directive_inputs() {
    assert!(manifest("appmanifest_410005.acf", TRUNCATED).is_err());
    for text in [
        "\"AppState\" { \"appid\" \"410001\" \"appID\" \"410002\" }",
        "\"AppState\" { \"appid\" \"410001\" \"x\" \"bad\\q\" }",
        "#include \"C:/private\"",
        "\"#base\" \"C:/private\"",
        "AppState { appid 410001 }",
        "\"x\" \"unterminated",
        "\"x\" { } }",
    ] {
        assert!(parse(text.as_bytes(), MANIFEST_BYTES).is_err(), "{text}");
    }
    assert_eq!(parse(b"\xff", MANIFEST_BYTES).unwrap_err(), Error::Encoding);
    assert_eq!(
        parse(b"\"x\" \"nul\0\"", MANIFEST_BYTES).unwrap_err(),
        Error::Encoding
    );
    let mut bom = b"\xef\xbb\xbf".to_vec();
    bom.extend_from_slice(INSTALLED);
    assert_eq!(
        manifest("appmanifest_410001.acf", &bom).unwrap().app_id,
        410001
    );
}
#[test]
fn folder_formats_are_candidates_only_and_hostile_entries_are_isolated() {
    let modern = folders(include_bytes!("fixtures/steam/libraryfolders-modern.vdf")).unwrap();
    assert_eq!(
        modern.candidates,
        vec!["C:\\FixtureSteam", "D:\\FixtureLibrary"]
    );
    assert_eq!(modern.rejected, 0);
    let legacy = folders(include_bytes!("fixtures/steam/libraryfolders-legacy.vdf")).unwrap();
    assert_eq!(legacy.candidates.len(), 2);
    assert_eq!(legacy.rejected, 0);
    let unsafe_folders =
        folders(include_bytes!("fixtures/steam/libraryfolders-unsafe.vdf")).unwrap();
    assert_eq!(unsafe_folders.candidates, vec!["D:\\ApprovedCandidateOnly"]);
    assert_eq!(unsafe_folders.rejected, 5);
    // The index's `apps` listing cannot produce any installation or account membership.
    assert!(modern.candidates.iter().all(|p| !p.contains("410006")));
}
#[derive(Deserialize)]
struct PathCase {
    value: String,
    valid: bool,
}
#[derive(Deserialize)]
struct Paths {
    components: Vec<PathCase>,
    libraries: Vec<PathCase>,
    metadata: Vec<PathCase>,
}
#[test]
fn malicious_paths_metadata_allowlist_and_windows_names_fail_closed() {
    let paths: Paths =
        serde_json::from_str(include_str!("fixtures/steam/path-cases.json")).unwrap();
    for c in paths.components {
        assert_eq!(
            component(&c.value).is_ok(),
            c.valid,
            "component {:?}",
            c.value
        );
    }
    for c in paths.libraries {
        assert_eq!(
            library_path(&c.value).is_ok(),
            c.valid,
            "library {:?}",
            c.value
        );
    }
    for c in paths.metadata {
        assert_eq!(
            allowed_relative_metadata(&c.value),
            c.valid,
            "metadata {:?}",
            c.value
        );
    }
    assert_eq!(component(&"🎮".repeat(128)), Err(Error::InvalidPath));
    assert_eq!(library_path("C:\\\\"), Err(Error::InvalidPath));
}
#[derive(Deserialize)]
struct AccountCase {
    known: Vec<String>,
    choice: Option<String>,
    expected: String,
}
#[derive(Deserialize)]
struct CacheCase {
    name: String,
    directory: bool,
    reparse: bool,
    expected: Option<u32>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AbsenceCase {
    complete: bool,
    manifest_missing: bool,
    propose_installation_missing: bool,
}
#[derive(Deserialize)]
struct Policy {
    accounts: Vec<AccountCase>,
    cache: Vec<CacheCase>,
    absence: Vec<AbsenceCase>,
    contexts: Vec<ContextCase>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContextCase {
    name: String,
    running: bool,
    reported_current: Option<String>,
    previous: String,
    family_hint: bool,
    cache_complete: bool,
}
fn policy() -> Policy {
    serde_json::from_str(include_str!("fixtures/steam/policy-cases.json")).unwrap()
}
#[test]
fn no_account_is_selected_from_directory_existence_or_most_recent_hints() {
    for case in policy().accounts {
        let known: Vec<_> = case.known.iter().map(String::as_str).collect();
        let result = account_scope(&known, case.choice.as_deref());
        match case.expected.as_str() {
            "selectionRequired" => assert_eq!(result, Err(Error::SelectionRequired)),
            "unattributed" => assert_eq!(result, Ok(AccountScope::UnattributedClient)),
            "selected" => assert_eq!(
                result,
                Ok(AccountScope::UserSelected(case.choice.as_deref().unwrap()))
            ),
            _ => panic!("Unknown fixture expectation"),
        }
        assert_eq!(
            manifest("appmanifest_410001.acf", INSTALLED)
                .unwrap()
                .ownership,
            Ownership::Unknown
        );
    }
}
#[test]
fn cached_uninstalled_and_family_ambiguous_candidates_never_prove_ownership() {
    for case in policy().cache {
        assert_eq!(
            cache_candidate(&case.name, case.directory, case.reparse),
            case.expected
        );
    }
    assert_eq!(
        cache_classification(),
        (
            Installation::Unknown,
            Visibility::CacheCandidate,
            Ownership::Unknown,
            true
        )
    );
    assert_ne!(
        cache_classification().0,
        Installation::ClientReportedPresent
    );
    // App 410006 occurs in the synthetic cache/index but has no valid manifest.
    // Even selecting fictional-A/B cannot establish who owns it or who shares it.
    assert_eq!(
        account_scope(&["fictional-A", "fictional-B"], Some("fictional-B")),
        Ok(AccountScope::UserSelected("fictional-B"))
    );
    assert_eq!(cache_classification().2, Ownership::Unknown);
}
#[test]
fn stopped_running_account_switch_and_family_contexts_cannot_upgrade_evidence() {
    let key = external_identity("client-stable-id", 410001).unwrap();
    for c in policy().contexts {
        assert!(!c.name.is_empty());
        let mut hints = vec![c.previous.as_str()];
        if c.running
            && let Some(current) = c.reported_current.as_deref()
        {
            hints.push(current);
        }
        assert_eq!(account_scope(&hints, None), Err(Error::SelectionRequired));
        assert_eq!(
            account_scope(&hints, Some("continue-unattributed")),
            Ok(AccountScope::UnattributedClient)
        );
        assert_eq!(
            manifest("appmanifest_410001.acf", INSTALLED)
                .unwrap()
                .ownership,
            Ownership::Unknown
        );
        assert_eq!(external_identity("client-stable-id", 410001).unwrap(), key);
        if c.family_hint {
            assert_eq!(cache_classification().2, Ownership::Unknown);
        }
        if !c.cache_complete {
            assert!(!may_propose_installation_missing(false, true));
        }
    }
}
#[test]
fn cache_removal_and_incomplete_scans_never_revoke_membership() {
    for c in policy().absence {
        assert_eq!(
            may_propose_installation_missing(c.complete, c.manifest_missing),
            c.propose_installation_missing
        );
    }
    assert_eq!(cache_classification().2, Ownership::Unknown);
}
#[test]
fn duplicate_appids_group_installations_and_titles_do_not_define_identity() {
    let rows = vec![
        (
            "root-a".into(),
            manifest("appmanifest_410001.acf", INSTALLED).unwrap(),
        ),
        (
            "root-a".into(),
            manifest("appmanifest_410001.acf", INSTALLED).unwrap(),
        ),
        (
            "root-b".into(),
            manifest("appmanifest_410001.acf", INSTALLED).unwrap(),
        ),
    ];
    let grouped = groups(&rows).unwrap();
    assert_eq!(grouped.len(), 1);
    assert_eq!(grouped[&410001].len(), 2);
    let key = external_identity("client-stable-id", 410001).unwrap();
    assert_eq!(
        key,
        (
            "steam.local.v1/client/client-stable-id".into(),
            "410001".into()
        )
    );
    let renamed = String::from_utf8(INSTALLED.to_vec())
        .unwrap()
        .replace("Café", "Renamed");
    assert_eq!(
        external_identity(
            "client-stable-id",
            manifest("appmanifest_410001.acf", renamed.as_bytes())
                .unwrap()
                .app_id
        )
        .unwrap(),
        key
    );
    assert_eq!(association(&[]), Association::NewRequired); // no title-match input exists
    assert_eq!(
        association(&["canonical-stable-id", "canonical-stable-id"]),
        Association::Existing("canonical-stable-id")
    );
    assert_eq!(
        association(&["one", "another"]),
        Association::ReviewConflict
    );
}
#[test]
fn appid_validation_filename_mismatch_and_missing_fields_are_safe() {
    for value in ["0", "01", "+1", "-1", "4294967296", "１", "1.0", ""] {
        assert!(app_id(value).is_err());
    }
    assert_eq!(app_id("4294967295"), Ok(u32::MAX));
    assert_eq!(
        manifest("appmanifest_410002.acf", INSTALLED).unwrap_err(),
        Error::InvalidId
    );
    assert!(manifest("appmanifest_410001.acf", b"\"AppState\" {}").is_err());
    let unsafe_dir = String::from_utf8(INSTALLED.to_vec())
        .unwrap()
        .replace("Synthetic Adventure", "..\\\\Escape");
    assert_eq!(
        manifest("appmanifest_410001.acf", unsafe_dir.as_bytes()).unwrap_err(),
        Error::InvalidPath
    );
}
#[test]
fn all_five_manual_overrides_win_including_null_and_unchanged_values() {
    let overrides: BTreeMap<_, _> = [
        ("title", Some("User title")),
        ("release_date", None),
        ("genre", None),
        ("developer", Some("Unchanged studio")),
        ("favorite", Some("0")),
    ]
    .into();
    let provider: BTreeMap<_, _> = [
        ("title", "Renamed provider title"),
        ("release_date", "2026-10-28"),
        ("genre", "New genre"),
        ("developer", "New studio"),
        ("favorite", "1"),
    ]
    .into();
    for (field, user) in &overrides {
        assert_eq!(
            effective_value(Some(*user), provider.get(field).copied()),
            user.map(str::to_owned)
        );
    }
    assert_eq!(
        effective_value(None, Some("New provider title")),
        Some("New provider title".into())
    );
    assert_eq!(
        effective_value(Some(None), Some("Provider must not fill this")),
        None
    );
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SnapshotCase {
    case: String,
    first: String,
    second: String,
    before_id: u64,
    after_id: u64,
    before_time: u64,
    after_time: u64,
    declared_length: u64,
    regular: bool,
    reparse: bool,
    expected: String,
}
#[test]
fn changing_truncated_oversized_and_reparse_snapshots_are_rejected() {
    let cases: Vec<SnapshotCase> =
        serde_json::from_str(include_str!("fixtures/steam/snapshot-cases.json")).unwrap();
    for c in cases {
        let before = Stamp {
            file_id: c.before_id,
            modified: c.before_time,
            length: c.declared_length,
            regular: c.regular,
            reparse: c.reparse,
        };
        let after = Stamp {
            file_id: c.after_id,
            modified: c.after_time,
            ..before
        };
        let actual = stable_snapshot(
            before,
            after,
            c.first.as_bytes(),
            c.second.as_bytes(),
            MANIFEST_BYTES,
        );
        match c.expected.as_str() {
            "ok" => assert_eq!(actual.unwrap(), c.first.as_bytes(), "{}", c.case),
            "changed" => assert_eq!(actual.unwrap_err(), Error::Changed, "{}", c.case),
            "unsafe" => assert_eq!(actual.unwrap_err(), Error::UnsafeSnapshot, "{}", c.case),
            "limit" => assert_eq!(actual.unwrap_err(), Error::Limit, "{}", c.case),
            _ => panic!("Unknown fixture expectation"),
        }
    }
}
struct CountingReader<'a>(&'a Cell<usize>);
impl Read for CountingReader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.0.set(self.0.get() + buf.len());
        buf.fill(b'x');
        Ok(buf.len())
    }
}
#[test]
fn file_token_depth_and_pair_budgets_bound_work() {
    let count = Cell::new(0);
    assert_eq!(
        read_bounded(CountingReader(&count), 1024).unwrap_err(),
        Error::Limit
    );
    assert_eq!(count.get(), 1025);
    assert_eq!(
        parse(&vec![b' '; MANIFEST_BYTES + 1], MANIFEST_BYTES).unwrap_err(),
        Error::Limit
    );
    let deep = format!("{}\"v\" \"text\"{}", "\"x\" {".repeat(18), "}".repeat(18));
    assert_eq!(
        parse(deep.as_bytes(), INDEX_BYTES).unwrap_err(),
        Error::Limit
    );
    let long = format!("\"x\" \"{}\"", "x".repeat(4097));
    assert_eq!(
        parse(long.as_bytes(), MANIFEST_BYTES).unwrap_err(),
        Error::Limit
    );
    let apps = (0..25000)
        .map(|i| format!("\"{}\" \"1000\" ", 410000 + i))
        .collect::<String>();
    let index = format!(
        "\"libraryfolders\" {{ \"0\" {{ \"path\" \"C:\\\\SyntheticSteam\" \"apps\" {{ {apps} }} }} }}"
    );
    assert_eq!(folders(index.as_bytes()).unwrap().candidates.len(), 1);
    let pairs = (0..32769)
        .map(|i| format!("\"k{i}\" \"v\"\n"))
        .collect::<String>();
    assert_eq!(
        parse(pairs.as_bytes(), INDEX_BYTES).unwrap_err(),
        Error::Limit
    );
    let roots = format!(
        "\"libraryfolders\" {{{}}}",
        (0..33)
            .map(|i| format!("\"{i}\" \"C:\\\\root{i}\" "))
            .collect::<String>()
    );
    assert_eq!(folders(roots.as_bytes()).unwrap_err(), Error::Limit);
}
#[derive(Deserialize)]
struct BinaryCase {
    kind: String,
    hex: String,
    expected: String,
}
#[test]
fn undocumented_binary_formats_are_only_gated_never_parsed_or_used_as_licenses() {
    let cases: Vec<BinaryCase> =
        serde_json::from_str(include_str!("fixtures/steam/binary-headers.json")).unwrap();
    for c in cases {
        let bytes = (0..c.hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&c.hex[i..i + 2], 16).unwrap())
            .collect::<Vec<_>>();
        let actual = binary_gate(&bytes, c.kind == "packageinfo");
        let expected = match c.expected.as_str() {
            "deferred39" => BinaryGate::KnownButDeferred(39),
            "deferred40" => BinaryGate::KnownButDeferred(40),
            "deferred41" => BinaryGate::KnownButDeferred(41),
            "unsupported" => BinaryGate::Unsupported,
            "truncated" => BinaryGate::Truncated,
            _ => panic!("Unknown fixture expectation"),
        };
        assert_eq!(actual, expected);
    }
}
#[test]
fn partial_preview_retains_valid_records_and_cancellation_discards_uncommitted_results() {
    let files = [
        ("appmanifest_410001.acf", Some(INSTALLED)),
        ("appmanifest_410005.acf", Some(TRUNCATED)),
        ("appmanifest_410006.acf", None),
        ("appmanifest_410003.acf", Some(NO_NAME)),
    ];
    let result = preview(&files, |_| false).unwrap();
    assert_eq!(result.items.len(), 2);
    assert_eq!(result.warnings.len(), 2);
    assert_eq!(result.warnings[1], (2, Error::Missing));
    assert_eq!(
        preview(&files, |index| index >= 2).unwrap_err(),
        Error::Cancelled
    );
    assert!(preview(&[], |_| false).unwrap().items.is_empty());
}
