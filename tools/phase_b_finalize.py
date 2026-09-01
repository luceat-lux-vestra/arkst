#!/usr/bin/env python3
from pathlib import Path
import re

# Temporary migration helper. It is deleted before the final strict review.

# 1) Replace the compactor's approximate fallback graph with the exact
# JDK 25 ResourceBundle.Control candidate algorithm (semantically transcribed).
generator = Path('tools/generate_jdk25_locale_data.py')
s = generator.read_text()
start = s.index('def is_ascii_alpha(value: str) -> bool:')
end = s.index('\ndef pool_section(', start)
replacement = r'''def is_ascii_alpha(value: str) -> bool:
    return bool(value) and value.isascii() and value.isalpha()


def is_variant_subtag(value: str) -> bool:
    return (
        5 <= len(value) <= 8 and value.isascii() and value.isalnum()
    ) or (
        len(value) == 4
        and value[0].isdigit()
        and value.isascii()
        and value.isalnum()
    )


def parse_profile(profile: str) -> tuple[str, str, str, list[str]]:
    if not profile:
        return "", "", "", []
    parts = profile.split("-")
    language = parts[0].lower()
    index = 1
    script = ""
    if index < len(parts) and len(parts[index]) == 4 and is_ascii_alpha(parts[index]):
        script = parts[index][0].upper() + parts[index][1:].lower()
        index += 1
    region = ""
    if index < len(parts) and (
        len(parts[index]) == 2 and is_ascii_alpha(parts[index])
        or len(parts[index]) == 3 and parts[index].isascii() and parts[index].isdigit()
    ):
        region = parts[index].upper()
        index += 1
    variants: list[str] = []
    while index < len(parts) and is_variant_subtag(parts[index]):
        variants.append(parts[index])
        index += 1
    return language, script, region, variants


def candidate_tag(language: str, script: str, region: str, variants: list[str]) -> str:
    if not language and not script and not region and not variants:
        return ""
    parts: list[str] = []
    if language:
        parts.append(language.lower())
    if script:
        parts.append(script[0].upper() + script[1:].lower())
    if region:
        parts.append(region.upper())
    valid_count = 0
    for variant in variants:
        if not is_variant_subtag(variant):
            break
        parts.append(variant)
        valid_count += 1
    if valid_count < len(variants):
        parts.extend(["x", "lvariant", *variants[valid_count:]])
    return "-".join(parts)


def default_candidates(
    language: str, script: str, region: str, variants: list[str]
) -> list[tuple[str, str, str, list[str]]]:
    prefixes = [variants[:count] for count in range(len(variants), 0, -1)]
    result: list[tuple[str, str, str, list[str]]] = []
    result.extend((language, script, region, prefix) for prefix in prefixes)
    if region:
        result.append((language, script, region, []))
    restart_region = region
    if script:
        result.append((language, script, "", []))
        if language == "zh" and not restart_region:
            if script == "Hans":
                restart_region = "CN"
            elif script == "Hant":
                restart_region = "TW"
        result.extend((language, "", restart_region, prefix) for prefix in prefixes)
        if restart_region:
            result.append((language, "", restart_region, []))
    if language:
        result.append((language, "", "", []))
    result.append(("", "", "", []))
    return result


def candidate_profiles(profile: str) -> list[str]:
    language, script, region, variants = parse_profile(profile)
    is_bokmal = False
    is_nynorsk = False
    if language == "no":
        if region == "NO" and variants == ["NY"]:
            variants = []
            is_nynorsk = True
        else:
            is_bokmal = True

    candidates: list[tuple[str, str, str, list[str]]]
    if language == "nb" or is_bokmal:
        base = default_candidates("nb", script, region, variants)
        candidates = []
        for candidate in base:
            lang, cand_script, cand_region, cand_variants = candidate
            if not lang:
                candidates.append(candidate)
                break
            other = ("no", cand_script, cand_region, cand_variants)
            if is_bokmal:
                candidates.extend([other, candidate])
            else:
                candidates.extend([candidate, other])
    elif language == "nn" or is_nynorsk:
        candidates = default_candidates("nn", script, region, variants)
        root_index = len(candidates) - 1
        candidates[root_index:root_index] = [
            ("no", "", "NO", ["NY"]),
            ("no", "", "NO", []),
            ("no", "", "", []),
        ]
    else:
        if language == "zh" and not script and region:
            if region in {"TW", "HK", "MO"}:
                script = "Hant"
            elif region in {"CN", "SG"}:
                script = "Hans"
        candidates = default_candidates(language, script, region, variants)

    result: list[str] = []
    for language, script, region, variants in candidates:
        tag = candidate_tag(language, script, region, variants)
        if tag not in result:
            result.append(tag)
    return result


def fallback_profiles(profile: str) -> list[str]:
    candidates = candidate_profiles(profile)
    return candidates[1:] if candidates and candidates[0] == profile else candidates
'''
s = s[:start] + replacement + s[end:]
# The runtime no longer consumes or documents an approximate fallback-order enum.
s = re.sub(
    r'\n        "pub const LOCALE_DISPLAY_FALLBACK_ORDER: &\[&str\] = &\[",\n'
    r'        \*\[f"    \{rust_string\(item\)\}," for item in DISPLAY_FALLBACK_ORDER\],\n'
    r'        "\];",',
    '', s, count=1)
# If the generated constant was rendered by a slightly different source layout,
# remove its source declaration too.
s = re.sub(
    r'\nDISPLAY_FALLBACK_ORDER = \(.*?\n\)\n', '\n', s, count=1, flags=re.S)
generator.write_text(s)

# 2) Runtime exact candidate graph. Use display/BaseLocale identity rather than
# canonical serialization identity, which matters for legacy no_NO_NY.
locale = Path('crates/scribium-engine/src/locale.rs')
s = locale.read_text()
start = s.index('fn display_data_value(parsed: &ParsedLanguageTag, key: &str)')
end = s.index('\nfn display_variant(', start)
rust = r'''#[derive(Debug, Clone, PartialEq, Eq)]
struct CandidateLocale {
    language: String,
    script: String,
    region: String,
    variants: Vec<String>,
}

fn candidate_tag(candidate: &CandidateLocale) -> String {
    if candidate.language.is_empty()
        && candidate.script.is_empty()
        && candidate.region.is_empty()
        && candidate.variants.is_empty()
    {
        return String::new();
    }
    let mut parts = Vec::new();
    if !candidate.language.is_empty() {
        parts.push(candidate.language.to_ascii_lowercase());
    }
    if !candidate.script.is_empty() {
        parts.push(titlecase_ascii(&candidate.script));
    }
    if !candidate.region.is_empty() {
        parts.push(candidate.region.to_ascii_uppercase());
    }
    let valid_count = candidate
        .variants
        .iter()
        .take_while(|variant| is_variant_subtag(variant))
        .count();
    parts.extend(candidate.variants[..valid_count].iter().cloned());
    if valid_count < candidate.variants.len() {
        parts.push("x".to_string());
        parts.push("lvariant".to_string());
        parts.extend(candidate.variants[valid_count..].iter().cloned());
    }
    parts.join("-")
}

fn default_candidate_locales(
    language: &str,
    script: &str,
    region: &str,
    variants: &[String],
) -> Vec<CandidateLocale> {
    let prefixes = (1..=variants.len())
        .rev()
        .map(|count| variants[..count].to_vec())
        .collect::<Vec<_>>();
    let mut result = Vec::new();
    for prefix in &prefixes {
        result.push(CandidateLocale {
            language: language.to_string(),
            script: script.to_string(),
            region: region.to_string(),
            variants: prefix.clone(),
        });
    }
    if !region.is_empty() {
        result.push(CandidateLocale {
            language: language.to_string(),
            script: script.to_string(),
            region: region.to_string(),
            variants: Vec::new(),
        });
    }
    let mut restart_region = region.to_string();
    if !script.is_empty() {
        result.push(CandidateLocale {
            language: language.to_string(),
            script: script.to_string(),
            region: String::new(),
            variants: Vec::new(),
        });
        if language == "zh" && restart_region.is_empty() {
            restart_region = match script {
                "Hans" => "CN".to_string(),
                "Hant" => "TW".to_string(),
                _ => String::new(),
            };
        }
        for prefix in &prefixes {
            result.push(CandidateLocale {
                language: language.to_string(),
                script: String::new(),
                region: restart_region.clone(),
                variants: prefix.clone(),
            });
        }
        if !restart_region.is_empty() {
            result.push(CandidateLocale {
                language: language.to_string(),
                script: String::new(),
                region: restart_region.clone(),
                variants: Vec::new(),
            });
        }
    }
    if !language.is_empty() {
        result.push(CandidateLocale {
            language: language.to_string(),
            script: String::new(),
            region: String::new(),
            variants: Vec::new(),
        });
    }
    result.push(CandidateLocale {
        language: String::new(),
        script: String::new(),
        region: String::new(),
        variants: Vec::new(),
    });
    result
}

fn display_candidate_profiles(parsed: &ParsedLanguageTag) -> Vec<String> {
    let language = parsed.display_language.as_str();
    let script = parsed.script.as_deref().unwrap_or("");
    let region = parsed.region.as_deref().unwrap_or("");
    let mut variants = parsed.display_variants.clone();
    let mut is_bokmal = false;
    let mut is_nynorsk = false;
    if language == "no" {
        if region == "NO" && variants == ["NY"] {
            variants.clear();
            is_nynorsk = true;
        } else {
            is_bokmal = true;
        }
    }

    let candidates = if language == "nb" || is_bokmal {
        let base = default_candidate_locales("nb", script, region, &variants);
        let mut result = Vec::new();
        for candidate in base {
            if candidate.language.is_empty() {
                result.push(candidate);
                break;
            }
            let mut other = candidate.clone();
            other.language = "no".to_string();
            if is_bokmal {
                result.push(other);
                result.push(candidate);
            } else {
                result.push(candidate);
                result.push(other);
            }
        }
        result
    } else if language == "nn" || is_nynorsk {
        let mut result = default_candidate_locales("nn", script, region, &variants);
        let root_index = result.len().saturating_sub(1);
        result.splice(
            root_index..root_index,
            [
                CandidateLocale {
                    language: "no".to_string(),
                    script: String::new(),
                    region: "NO".to_string(),
                    variants: vec!["NY".to_string()],
                },
                CandidateLocale {
                    language: "no".to_string(),
                    script: String::new(),
                    region: "NO".to_string(),
                    variants: Vec::new(),
                },
                CandidateLocale {
                    language: "no".to_string(),
                    script: String::new(),
                    region: String::new(),
                    variants: Vec::new(),
                },
            ],
        );
        result
    } else {
        let mut inferred_script = script.to_string();
        if language == "zh" && inferred_script.is_empty() && !region.is_empty() {
            inferred_script = match region {
                "TW" | "HK" | "MO" => "Hant".to_string(),
                "CN" | "SG" => "Hans".to_string(),
                _ => String::new(),
            };
        }
        default_candidate_locales(language, &inferred_script, region, &variants)
    };

    let mut profiles = Vec::new();
    for candidate in candidates {
        let profile = candidate_tag(&candidate);
        if !profiles.contains(&profile) {
            profiles.push(profile);
        }
    }
    profiles
}

fn display_data_value(parsed: &ParsedLanguageTag, key: &str) -> Option<&'static str> {
    let snapshot = DisplaySnapshot::parse(LOCALE_DISPLAY_DATA)?;
    display_candidate_profiles(parsed).iter().find_map(|profile| {
        snapshot
            .profile_id(profile)
            .and_then(|profile_id| snapshot.resolve_profile(profile_id, key))
    })
}
'''
s = s[:start] + rust + s[end:]
# Generated approximate fallback order is gone.
s = s.replace(', LOCALE_DISPLAY_FALLBACK_ORDER', '')
s = s.replace('LOCALE_DISPLAY_FALLBACK_ORDER, ', '')
locale.write_text(s)
