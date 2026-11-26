use super::*;

/// A deterministic pseudo-random string of `n` characters of `alphabet`
/// (splitmix64), so the tests hold no literal secret for a scanner to find.
pub(crate) fn fake(seed: u64, alphabet: &str, n: usize) -> String {
    let chars: Vec<char> = alphabet.chars().collect();
    let mut state = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15).wrapping_add(1);
    (0..n)
        .map(|_| {
            state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            chars[(z % chars.len() as u64) as usize]
        })
        .collect()
}

const ALNUM: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
const URLSAFE: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
const BASE64: &str = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
const HEX: &str = "0123456789abcdef";

/// No run of 8 characters of the secret survives in `text`.
fn leaks(text: &str, secret: &str) -> bool {
    let chars: Vec<char> = secret.chars().collect();
    chars
        .windows(8)
        .any(|w| text.contains(&w.iter().collect::<String>()))
}

fn assert_masked(secret: &str, context: &str) {
    let given = context.replace("SECRET", secret);
    let got = mask(&given);
    assert!(got.changed(), "not masked: {given}");
    assert!(!leaks(&got.text, secret), "leaked: {}", got.text);
    assert!(got.text.contains("[masked:"));
    let again = mask(&got.text);
    assert_eq!(again.text, got.text, "not idempotent");
    assert_eq!(again.count, 0);
}

// ---- masked wherever it sits

#[test]
fn test_a_bare_token_is_masked_whole() {
    let t = fake(1, ALNUM, 48);
    assert_eq!(
        mask(&t),
        Masked {
            text: marker(48),
            count: 1
        }
    );
}

#[test]
fn test_tokens_are_masked_in_every_position() {
    let t = fake(2, ALNUM, 56);
    for ctx in [
        "SECRET",
        "run SECRET now",
        "SECRET first",
        "last SECRET",
        "export API_TOKEN=SECRET",
        "curl -H \"Authorization: Bearer SECRET\" https://example.com/v1",
        "git clone https://user:SECRET@example.com/repo.git",
        "tool --token=SECRET --verbose",
        "tool --token SECRET --verbose",
        "{\"key\": \"SECRET\"}",
        "line one\nSECRET\nline three",
        "tab\tSECRET\tend",
    ] {
        assert_masked(&t, ctx);
    }
}

#[test]
fn test_the_value_after_an_equals_sign_is_judged_alone_and_the_sign_kept() {
    let t = fake(3, ALNUM, 56);
    let got = mask(&format!("--token={t} next KEY={t}"));
    assert_eq!(got.text, "--token=[masked:56] next KEY=[masked:56]");
    assert_eq!(got.count, 2);
}

#[test]
fn test_short_random_tokens_are_masked() {
    for t in ["aB3xY9", "Zk2LmX9v", "Tr0ub4dX", "q7Zk2LmX", "Zk2LmX9vRt4B"] {
        assert_masked(t, "login SECRET now");
    }
}

#[test]
fn test_short_ordinary_words_and_flags_are_left_readable() {
    for t in [
        "release",
        "--all",
        "origin",
        "main",
        "src/lib.rs",
        "HEAD~1",
        "terminal",
        "problems",
        "Cargo.toml",
        "README.md",
        "--release",
        "rust-support",
        "v1.2.3",
    ] {
        assert_eq!(mask(t).count, 0, "{t}");
    }
}

#[test]
fn test_a_name_with_digits_and_capitals_is_a_documented_over_mask() {
    assert_eq!(mask("getUserById2HandlerFactory").text, marker(26));
}

#[test]
fn test_each_block_is_judged_on_its_own() {
    let a = fake(4, ALNUM, 52);
    let b = fake(5, URLSAFE, 60);
    let got = mask(&format!("one {a} two {b} three"));
    assert_eq!(got.text, "one [masked:52] two [masked:60] three");
    assert_eq!(got.count, 2);
}

#[test]
fn test_url_safe_and_standard_base64_are_masked() {
    assert_masked(&fake(6, URLSAFE, 64), "x SECRET y");
    assert_masked(&fake(7, BASE64, 64), "x SECRET y");
}

#[test]
fn test_hex_keys_of_other_lengths_are_masked() {
    for (seed, n) in [(8, 32), (9, 33), (10, 48), (11, 63), (12, 128)] {
        assert_masked(&fake(seed, HEX, n), "digest SECRET end");
    }
}

#[test]
fn test_git_and_sha256_ids_are_left_readable() {
    for (seed, n) in [(20, 40), (21, 64)] {
        let id = fake(seed, HEX, n);
        let id_upper = id.to_uppercase();
        for given in [id.clone(), id_upper, format!("git show {id} --stat")] {
            assert_eq!(
                mask(&given),
                Masked {
                    text: given.clone(),
                    count: 0
                },
                "{given}"
            );
        }
    }
}

#[test]
fn test_a_block_of_a_script_without_case_is_not_judged_and_does_not_panic() {
    let t: String = ('\u{4e00}'..).take(40).chain(['7']).collect();
    // Scripts without case are not judged: the text is kept, nothing panics.
    assert_eq!(mask(&format!("a {t} b")).count, 0);
}

// ---- left readable

#[test]
fn test_ordinary_commands_paths_and_prose_are_unchanged() {
    for text in [
        "",
        "cargo build --release --workspace",
        "cargo test -p secret-mask -- --nocapture",
        "git commit -m \"fix the parser and add tests\"",
        "git checkout -b feat/secret-mask origin/main",
        "git log --oneline --graph --decorate HEAD~1..HEAD",
        "git diff --stat origin/main...HEAD",
        "git push --set-upstream origin feat/secret-mask",
        "git rebase -i HEAD~3",
        "git stash list",
        "just test-pkg duration-text coded-diagnostic",
        "just lint-deps && just fmt-check",
        "grep -rn \"fn main\" crates/core/domain/ --include=*.rs",
        "sed -n 1,40p crates/adapters/driven/file-system/src/lib.rs",
        "cat /home/babbitt/workspace/worktrees/rust-support--secret-mask/Cargo.toml",
        "find . -name \"*.test.rs\" -not -path \"./target/*\"",
        "curl -sL -m 30 https://raw.githubusercontent.com/gitleaks/gitleaks/master/LICENSE",
        "curl -H \"Content-Type: application/json\" https://api.example.com/v1/items?limit=20",
        "docker run --rm -it -v $PWD:/work -w /work rust:1.96 cargo check",
        "kubectl get pods -n production -o wide",
        "npm install --save-dev typescript@5.4.2 eslint",
        "python3 -m venv .venv && source .venv/bin/activate",
        "export RUST_BACKTRACE=1 CARGO_INCREMENTAL=0",
        "systemctl --user status demo-daemon.service",
        "demo list --format json --all",
        "demo assemble --profile default --format text",
        "tar -czf backup-2026-11-25.tar.gz crates/core/domain/secret-mask",
        "rg -n \"TODO|FIXME\" --glob '*.rs' crates/",
        "ps aux | grep claude-opus-4-5 | head -n 5",
        "echo \"Done: 3 tests passed, 0 failed, finished in 0.04s\"",
        "demo list --term crates/core/domain/secret-mask/src/lib.rs",
        "gh pr create --title \"Add secret-mask crate\" --body \"Masks entropy blocks\"",
        "rm -rf target/debug/incremental",
        "chmod 600 ~/.demo/daemon/daemon.discovery",
        "the quick brown fox jumps over the lazy dog, again and again",
    ] {
        assert_eq!(
            mask(text),
            Masked {
                text: text.to_owned(),
                count: 0
            },
            "{text}"
        );
    }
}

#[test]
fn test_a_uuid_is_left_readable() {
    let u = "550e8400-e29b-41d4-a716-446655440000";
    assert_eq!(mask(&format!("id {u}")).text, format!("id {u}"));
}

#[test]
fn test_a_short_hex_commit_id_is_left_readable() {
    assert_eq!(mask("git show f48afab4c").count, 0);
}

#[test]
fn test_low_entropy_secrets_are_known_misses() {
    for text in [
        "mysql --password=hunter2",
        "login password1",
        "key=abcd1234",
        "pw: correcthorse",
    ] {
        assert_eq!(mask(text).count, 0, "{text}");
    }
}

// ---- the text around

#[test]
fn test_whitespace_is_kept_exactly() {
    let a = fake(11, ALNUM, 50);
    let b = fake(12, ALNUM, 50);
    let given = format!("  lead\t\t{a}\r\n\n  mid  {b}\u{a0}tail \n");
    let want = "  lead\t\t[masked:50]\r\n\n  mid  [masked:50]\u{a0}tail \n";
    assert_eq!(mask(&given).text, want);
}

#[test]
fn test_only_whitespace_is_unchanged() {
    assert_eq!(mask(" \t\n ").text, " \t\n ");
}

#[test]
fn test_the_marker_names_the_length_in_characters() {
    assert_eq!(marker(0), "[masked:0]");
    assert_eq!(marker(48), "[masked:48]");
    assert_eq!(mask(&fake(30, ALNUM, 17)).text, marker(17));
}

#[test]
fn test_a_marker_is_never_masked_again() {
    for n in [0usize, 5, 12, 48, 123_456_789, usize::MAX] {
        let m = marker(n);
        assert_eq!(mask(&m).count, 0, "{m}");
        assert_eq!(mask(&format!("a {m} b")).text, format!("a {m} b"));
        assert_eq!(mask(&format!("KEY={m}")).text, format!("KEY={m}"));
    }
}

#[test]
fn test_text_that_only_looks_like_a_marker_is_judged_as_text() {
    let t = fake(31, ALNUM, 48);
    assert_eq!(mask(&format!("[masked:{t}]")).text, marker(48 + 9));
}

#[test]
fn test_changed_follows_the_count() {
    assert!(!mask("nothing here").changed());
    assert!(mask(&fake(13, ALNUM, 50)).changed());
}

// ---- properties

#[test]
fn test_random_tokens_in_prose_are_always_masked_and_never_leak() {
    for seed in 100..400u64 {
        let (alphabet, n) = match seed % 4 {
            0 => (ALNUM, 40 + (seed % 40) as usize),
            1 => (URLSAFE, 40 + (seed % 40) as usize),
            2 => (BASE64, 40 + (seed % 40) as usize),
            _ => (HEX, 41 + (seed % 22) as usize),
        };
        let t = fake(seed, alphabet, n);
        assert_masked(&t, "the build said SECRET twice, then stopped.");
    }
}

#[test]
fn test_random_text_never_panics_and_is_idempotent() {
    let alphabet = "ab1 \t\n\u{e9}\u{4e2d}\u{a0}-_/=+:.\"'";
    for seed in 0..300u64 {
        let given = fake(seed, alphabet, (seed % 90) as usize);
        let once = mask(&given);
        assert_eq!(mask(&once.text).text, once.text);
    }
}

// ---- the rate at which random tokens are masked, per length (fixed seeds)

/// The share of 400 random tokens of `n` characters of `alphabet` masked.
fn rate(alphabet: &str, n: usize) -> f64 {
    let hit = (0..400u64)
        .filter(|&s| mask(&fake(1_000 + s * 131 + n as u64, alphabet, n)).changed())
        .count();
    hit as f64 / 400.0
}

#[test]
fn test_random_62_symbol_tokens_of_every_length_from_12_are_masked_90_percent_or_more() {
    for n in (12..=64).chain([80, 96, 128, 200]) {
        let r = rate(ALNUM, n);
        assert!(r >= 0.90, "length {n}: {r}");
    }
}

#[test]
fn test_url_safe_and_base64_tokens_are_masked_90_percent_or_more_from_16() {
    for n in (16..=64).step_by(4) {
        assert!(rate(URLSAFE, n) >= 0.90, "url-safe {n}");
        assert!(rate(BASE64, n) >= 0.90, "base64 {n}");
    }
}

#[test]
fn test_short_random_tokens_are_masked_less_often_but_often() {
    assert!(rate(ALNUM, 6) >= 0.50);
    assert!(rate(ALNUM, 8) >= 0.65);
    assert!(rate(ALNUM, 10) >= 0.75);
    assert!(rate(ALNUM, 11) >= 0.80);
}

#[test]
fn test_ordinary_words_of_every_length_are_never_masked() {
    for w in [
        "internationalization",
        "troubleshooting-guide",
        "documentation-generator",
        "mutation-testing-framework",
        "deprecated_function_name",
        "configuration.yaml",
    ] {
        assert_eq!(mask(w).count, 0, "{w}");
    }
}

// ---- a token inside a URL or a path

#[test]
fn test_a_token_in_url_userinfo_is_masked_alone() {
    let t = fake(40, ALNUM, 56);
    let got = mask(&format!("git clone https://user:{t}@example.com/repo.git"));
    assert_eq!(
        got.text,
        format!("git clone https://user:{}@example.com/repo.git", marker(56))
    );
    assert_eq!(got.count, 1);
}

#[test]
fn test_a_token_in_a_path_is_masked_whole_or_alone_never_shown() {
    assert_masked(&fake(41, ALNUM, 40), "ls /srv/data/SECRET/files");
}

#[test]
fn test_a_base64_secret_with_slashes_is_masked_whole() {
    let t = fake(42, BASE64, 64);
    assert!(
        t.contains('/') || t.contains('+'),
        "the seed must give a symbol"
    );
    assert_eq!(mask(&t).text, marker(64));
}
