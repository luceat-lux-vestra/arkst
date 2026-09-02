//! Deterministic locale records used by the `.doclang` slice.
//!
//! Quarkdown v2.5.1 delegates locale lookup to `java.util.Locale`. The
//! platform-neutral evaluator cannot use that host database, so the complete
//! available-locale snapshot from the pinned reference JDK is checked in as
//! generated Rust data. Runtime lookup has no JVM, OS, ICU, filesystem, or
//! network dependency.

use crate::unicode_case::{simple_lowercase, simple_uppercase};
use arkst_ir::IrDocumentLocale;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LocaleRecord {
    tag: &'static str,
    display_name: &'static str,
    localized_name: &'static str,
    code: &'static str,
    script: &'static str,
    country_code: &'static str,
    variant: &'static str,
    localized_country_name: &'static str,
}

include!("locale_data.rs");

const LOCALE_DISPLAY_DATA: &[u8] = include_bytes!("../data/jdk25_locale_display.bin");
const LOCALE_DISPLAY_MAGIC: &[u8; 4] = b"SCLD";
const LOCALE_DISPLAY_HEADER_SIZE: usize = 72;
const LOCALE_DISPLAY_HEADER_FIELDS: usize = 17;

#[derive(Debug, Clone, Copy)]
struct DisplaySnapshot<'a> {
    data: &'a [u8],
    profile_pool: DisplayStringPool<'a>,
    key_pool: DisplayStringPool<'a>,
    value_pool: DisplayStringPool<'a>,
    profile_ranges_offset: usize,
    fallback_offset: usize,
    fallback_id_offset: usize,
    record_offset: usize,
    profile_count: usize,
    key_count: usize,
    value_count: usize,
    record_count: usize,
    fallback_count: usize,
}

#[derive(Debug, Clone, Copy)]
struct DisplayStringPool<'a> {
    data: &'a [u8],
    offset: usize,
    length: usize,
    count: usize,
}

impl<'a> DisplayStringPool<'a> {
    fn new(data: &'a [u8], offset: usize, length: usize, count: usize) -> Option<Self> {
        let offsets_size = count.checked_add(1)?.checked_mul(4)?;
        let section_end = offset.checked_add(length)?;
        let offsets_end = offset.checked_add(offsets_size)?;
        if offset < LOCALE_DISPLAY_HEADER_SIZE
            || offsets_end > section_end
            || section_end > data.len()
        {
            return None;
        }
        Some(Self {
            data,
            offset,
            length,
            count,
        })
    }

    fn offset(&self, index: usize) -> Option<usize> {
        let offset = self.offset.checked_add(index.checked_mul(4)?)?;
        read_u32(self.data, offset).and_then(|value| usize::try_from(value).ok())
    }

    fn get(&self, index: usize) -> Option<&'a str> {
        if index >= self.count {
            return None;
        }
        let bytes_start = self
            .offset
            .checked_add(self.count.checked_add(1)?.checked_mul(4)?)?;
        let bytes_end = self.offset.checked_add(self.length)?;
        let start = bytes_start.checked_add(self.offset(index)?)?;
        let end = bytes_start.checked_add(self.offset(index.checked_add(1)?)?)?;
        if start > end || end > bytes_end {
            return None;
        }
        std::str::from_utf8(self.data.get(start..end)?).ok()
    }
}

impl<'a> DisplaySnapshot<'a> {
    fn parse(data: &'a [u8]) -> Option<Self> {
        if data.len() < LOCALE_DISPLAY_HEADER_SIZE
            || data.get(..4)? != LOCALE_DISPLAY_MAGIC
            || LOCALE_DISPLAY_HEADER_FIELDS * 4 + 4 != LOCALE_DISPLAY_HEADER_SIZE
        {
            return None;
        }
        let format_version = read_u32(data, 4)?;
        if format_version != LOCALE_DISPLAY_COMPACT_FORMAT_VERSION {
            return None;
        }
        let profile_count = usize::try_from(read_u32(data, 8)?).ok()?;
        let key_count = usize::try_from(read_u32(data, 12)?).ok()?;
        let value_count = usize::try_from(read_u32(data, 16)?).ok()?;
        let record_count = usize::try_from(read_u32(data, 20)?).ok()?;
        let sections = [
            (read_u32(data, 24)?, read_u32(data, 28)?),
            (read_u32(data, 32)?, read_u32(data, 36)?),
            (read_u32(data, 40)?, read_u32(data, 44)?),
            (read_u32(data, 48)?, read_u32(data, 52)?),
            (read_u32(data, 56)?, read_u32(data, 60)?),
            (read_u32(data, 64)?, read_u32(data, 68)?),
        ];
        let mut bounds = [(0usize, 0usize); 6];
        for (index, (offset, length)) in sections.into_iter().enumerate() {
            let offset = usize::try_from(offset).ok()?;
            let length = usize::try_from(length).ok()?;
            let end = offset.checked_add(length)?;
            if offset < LOCALE_DISPLAY_HEADER_SIZE || end > data.len() {
                return None;
            }
            bounds[index] = (offset, end);
        }
        for (index, (left_start, left_end)) in bounds.into_iter().enumerate() {
            if bounds[index + 1..]
                .iter()
                .any(|(right_start, right_end)| left_start < *right_end && *right_start < left_end)
            {
                return None;
            }
        }
        let ranges_length = usize::try_from(sections[3].1).ok()?;
        let fallback_length = usize::try_from(sections[4].1).ok()?;
        let records_length = usize::try_from(sections[5].1).ok()?;
        let fallback_range_bytes = read_count_bytes(profile_count.checked_add(1)?)?;
        if fallback_length < fallback_range_bytes
            || (fallback_length - fallback_range_bytes) % 4 != 0
        {
            return None;
        }
        if ranges_length != read_count_bytes(profile_count.checked_add(1)?)?
            || records_length != record_count.checked_mul(8)?
        {
            return None;
        }
        Some(Self {
            data,
            profile_pool: DisplayStringPool::new(
                data,
                usize::try_from(sections[0].0).ok()?,
                usize::try_from(sections[0].1).ok()?,
                profile_count,
            )?,
            key_pool: DisplayStringPool::new(
                data,
                usize::try_from(sections[1].0).ok()?,
                usize::try_from(sections[1].1).ok()?,
                key_count,
            )?,
            value_pool: DisplayStringPool::new(
                data,
                usize::try_from(sections[2].0).ok()?,
                usize::try_from(sections[2].1).ok()?,
                value_count,
            )?,
            profile_ranges_offset: usize::try_from(sections[3].0).ok()?,
            fallback_offset: usize::try_from(sections[4].0).ok()?,
            fallback_id_offset: usize::try_from(sections[4].0)
                .ok()?
                .checked_add(fallback_range_bytes)?,
            record_offset: usize::try_from(sections[5].0).ok()?,
            profile_count,
            key_count,
            value_count,
            record_count,
            fallback_count: (fallback_length - fallback_range_bytes) / 4,
        })
    }

    fn profile_id(&self, profile: &str) -> Option<usize> {
        find_string_id(&self.profile_pool, profile)
    }

    fn key_id(&self, key: &str) -> Option<usize> {
        find_string_id(&self.key_pool, key)
    }

    fn profile_range(&self, profile_id: usize) -> Option<(usize, usize)> {
        if profile_id >= self.profile_count {
            return None;
        }
        let start = usize::try_from(read_u32(
            self.data,
            self.profile_ranges_offset
                .checked_add(profile_id.checked_mul(4)?)?,
        )?)
        .ok()?;
        let end = usize::try_from(read_u32(
            self.data,
            self.profile_ranges_offset
                .checked_add(profile_id.checked_add(1)?.checked_mul(4)?)?,
        )?)
        .ok()?;
        (start <= end && end <= self.record_count).then_some((start, end))
    }

    fn fallback_range(&self, profile_id: usize) -> Option<(usize, usize)> {
        if profile_id >= self.profile_count {
            return None;
        }
        let start = usize::try_from(read_u32(
            self.data,
            self.fallback_offset
                .checked_add(profile_id.checked_mul(4)?)?,
        )?)
        .ok()?;
        let end = usize::try_from(read_u32(
            self.data,
            self.fallback_offset
                .checked_add(profile_id.checked_add(1)?.checked_mul(4)?)?,
        )?)
        .ok()?;
        (start <= end && end <= self.fallback_count).then_some((start, end))
    }

    fn fallback_id(&self, index: usize) -> Option<usize> {
        if index >= self.fallback_count {
            return None;
        }
        usize::try_from(read_u32(
            self.data,
            self.fallback_id_offset.checked_add(index.checked_mul(4)?)?,
        )?)
        .ok()
    }

    fn record(&self, index: usize) -> Option<(usize, usize)> {
        if index >= self.record_count {
            return None;
        }
        let offset = self.record_offset.checked_add(index.checked_mul(8)?)?;
        Some((
            usize::try_from(read_u32(self.data, offset)?).ok()?,
            usize::try_from(read_u32(self.data, offset.checked_add(4)?)?).ok()?,
        ))
    }

    fn find_record(&self, profile_id: usize, key_id: usize) -> Option<&'a str> {
        if profile_id >= self.profile_count || key_id >= self.key_count {
            return None;
        }
        let (start, end) = self.profile_range(profile_id)?;
        let mut low = start;
        let mut high = end;
        while low < high {
            let middle = low + (high - low) / 2;
            let (candidate, value_id) = self.record(middle)?;
            if candidate < key_id {
                low = middle.checked_add(1)?;
            } else {
                high = middle;
            }
            if candidate == key_id {
                return (value_id < self.value_count)
                    .then(|| self.value_pool.get(value_id))
                    .flatten();
            }
        }
        None
    }

    fn resolve_profile(&self, profile_id: usize, key: &str) -> Option<&'a str> {
        self.resolve_profile_bounded(profile_id, key, 0)
    }

    fn resolve_profile_bounded(
        &self,
        profile_id: usize,
        key: &str,
        depth: usize,
    ) -> Option<&'a str> {
        // A valid generated graph is acyclic. The bound also makes a
        // corrupted embedded blob fail closed instead of recursing forever.
        if depth > self.profile_count {
            return None;
        }
        let key_id = self.key_id(key)?;
        if let Some(value) = self.find_record(profile_id, key_id) {
            return Some(value);
        }
        let (start, end) = self.fallback_range(profile_id)?;
        (start..end).find_map(|index| {
            self.fallback_id(index)
                .and_then(|fallback_id| self.resolve_profile_bounded(fallback_id, key, depth + 1))
        })
    }

    #[cfg(test)]
    fn validate(&self) -> Result<(), &'static str> {
        validate_string_pool(&self.profile_pool, true)?;
        validate_string_pool(&self.key_pool, true)?;
        validate_string_pool(&self.value_pool, true)?;
        let mut previous_range = 0;
        for profile_id in 0..=self.profile_count {
            let offset = self
                .profile_ranges_offset
                .checked_add(profile_id.checked_mul(4).ok_or("range overflow")?)
                .ok_or("range overflow")?;
            let value = usize::try_from(read_u32(self.data, offset).ok_or("range bounds")?)
                .map_err(|_| "range conversion")?;
            if value < previous_range || value > self.record_count {
                return Err("profile ranges");
            }
            previous_range = value;
        }
        let mut previous_fallback_range = 0;
        for profile_id in 0..=self.profile_count {
            let offset = self
                .fallback_offset
                .checked_add(profile_id.checked_mul(4).ok_or("fallback range overflow")?)
                .ok_or("fallback range overflow")?;
            let value =
                usize::try_from(read_u32(self.data, offset).ok_or("fallback range bounds")?)
                    .map_err(|_| "fallback range conversion")?;
            if value < previous_fallback_range || value > self.fallback_count {
                return Err("fallback ranges");
            }
            previous_fallback_range = value;
        }
        for index in 0..self.fallback_count {
            let fallback_id = self.fallback_id(index).ok_or("fallback ID bounds")?;
            if fallback_id >= self.profile_count {
                return Err("fallback ID");
            }
        }
        for profile_id in 0..self.profile_count {
            let (start, end) = self.profile_range(profile_id).ok_or("profile range")?;
            let mut previous_key = None;
            for index in start..end {
                let (key_id, value_id) = self.record(index).ok_or("record bounds")?;
                if key_id >= self.key_count || value_id >= self.value_count {
                    return Err("record ID");
                }
                if previous_key.is_some_and(|previous| key_id <= previous) {
                    return Err("record order");
                }
                previous_key = Some(key_id);
            }
        }
        Ok(())
    }
}

fn read_count_bytes(count: usize) -> Option<usize> {
    count.checked_mul(4)
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let bytes = data.get(offset..offset.checked_add(4)?)?;
    Some(u32::from_le_bytes(bytes.try_into().ok()?))
}

fn find_string_id(pool: &DisplayStringPool<'_>, value: &str) -> Option<usize> {
    let mut low = 0;
    let mut high = pool.count;
    while low < high {
        let middle = low + (high - low) / 2;
        let candidate = pool.get(middle)?;
        match candidate.cmp(value) {
            std::cmp::Ordering::Less => low = middle.checked_add(1)?,
            std::cmp::Ordering::Equal => return Some(middle),
            std::cmp::Ordering::Greater => high = middle,
        }
    }
    None
}

#[cfg(test)]
fn validate_string_pool(
    pool: &DisplayStringPool<'_>,
    require_sorted: bool,
) -> Result<(), &'static str> {
    let mut previous_offset = 0;
    let mut previous_value = None;
    for index in 0..=pool.count {
        let offset = pool.offset(index).ok_or("string offset bounds")?;
        let bytes_start = pool
            .offset
            .checked_add(
                pool.count
                    .checked_add(1)
                    .ok_or("string count")?
                    .checked_mul(4)
                    .ok_or("string size")?,
            )
            .ok_or("string start")?;
        let bytes_length = pool
            .offset
            .checked_add(pool.length)
            .ok_or("string end")?
            .checked_sub(bytes_start)
            .ok_or("string section")?;
        if offset < previous_offset || offset > bytes_length {
            return Err("string offsets");
        }
        if index < pool.count {
            let current = pool.get(index).ok_or("string UTF-8")?;
            if require_sorted && previous_value.is_some_and(|previous| current <= previous) {
                return Err("string order");
            }
            previous_value = Some(current);
        }
        previous_offset = offset;
    }
    Ok(())
}

/// Resolves an English display name before a canonical language tag.
///
/// Name records retain the exact iteration order returned by the pinned JVM's
/// `Locale.getAvailableLocales()`. That order is part of the name-first
/// contract whenever the English display-name audit finds a collision;
/// canonical tag records are a separate deduplicated index.
pub(crate) fn resolve(identifier: &str) -> Option<IrDocumentLocale> {
    resolve_detailed(identifier).map(|locale| IrDocumentLocale {
        tag: locale.tag,
        localized_name: locale.localized_name,
    })
}

fn string_equals_ignore_case(left: &str, right: &str) -> bool {
    let mut left_chars = left.chars();
    let mut right_chars = right.chars();
    loop {
        match (left_chars.next(), right_chars.next()) {
            (Some(left), Some(right)) if char_equals_ignore_case(left, right) => {}
            (None, None) => return true,
            _ => return false,
        }
    }
}

fn char_equals_ignore_case(left: char, right: char) -> bool {
    if left == right {
        return true;
    }
    let left_upper = simple_uppercase(left);
    let right_upper = simple_uppercase(right);
    left_upper == right_upper || simple_lowercase(left_upper) == simple_lowercase(right_upper)
}

/// JDK `LocaleUtils.toLowerString` semantics used by the
/// `LanguageTag.LEGACY` lookup: only ASCII A-Z are lowercased. This must not
/// reuse the Unicode-aware comparator used by Quarkdown's name-first lookup.
fn ascii_tag_equals_ignore_case(left: &str, right: &str) -> bool {
    left.eq_ignore_ascii_case(right)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParsedExtension {
    singleton: char,
    subtags: Vec<String>,
}

/// Bounded model of `sun.util.locale.LanguageTag.parse(..., lenient=true)`.
/// Values are intentionally kept separate from the builder and serializer:
/// this stage only consumes the valid prefix and does not promote extlangs,
/// extract `lvariant`, or apply Java compatibility extensions.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LanguageTagParts {
    language: String,
    extlangs: Vec<String>,
    script: String,
    region: String,
    variants: Vec<String>,
    extensions: Vec<ParsedExtension>,
    private_use: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct LocaleExtensionsModel {
    regular: Vec<ParsedExtension>,
    unicode_attributes: Vec<String>,
    unicode_keywords: Vec<(String, String)>,
    private_use: Vec<String>,
}

impl LocaleExtensionsModel {
    fn is_empty(&self) -> bool {
        self.regular.is_empty()
            && self.unicode_attributes.is_empty()
            && self.unicode_keywords.is_empty()
            && self.private_use.is_empty()
    }
}

/// BaseLocale identity plus the normalized LocaleExtensions model produced by
/// `InternalLocaleBuilder.setLanguageTag` and `LocaleExtensions`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct LocaleIdentity {
    language: String,
    script: String,
    region: String,
    variant: Vec<String>,
    extensions: LocaleExtensionsModel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ResolvedLocale {
    base: LocaleIdentity,
    tag: String,
    code: String,
    country_code: Option<String>,
    display_name: String,
    localized_name: String,
    localized_country_name: Option<String>,
    short_tag: String,
}

fn parse_jdk_language_tag(input: &str) -> LanguageTagParts {
    let mapped = legacy_language_tag(input).unwrap_or(input);
    let parts: Vec<&str> = mapped.split('-').collect();
    let language = parts
        .first()
        .copied()
        .filter(|part| is_language_subtag(part))
        .map(str::to_string)
        .unwrap_or_default();

    if language.is_empty() {
        let mut parse_error = false;
        return LanguageTagParts {
            language,
            extlangs: Vec::new(),
            script: String::new(),
            region: String::new(),
            variants: Vec::new(),
            extensions: Vec::new(),
            private_use: parse_private_use_subtags(&parts, 0, &mut parse_error),
        };
    }
    let mut index = 1;

    let mut extlangs = Vec::new();
    while extlangs.len() < 3 {
        let Some(part) = parts.get(index).copied() else {
            break;
        };
        if !is_extlang_subtag(part) {
            break;
        }
        extlangs.push(part.to_string());
        index += 1;
    }

    let script = parts
        .get(index)
        .copied()
        .filter(|part| is_script_subtag(part))
        .map(str::to_string)
        .unwrap_or_default();
    if !script.is_empty() {
        index += 1;
    }

    let region = parts
        .get(index)
        .copied()
        .filter(|part| is_region_subtag(part))
        .map(str::to_string)
        .unwrap_or_default();
    if !region.is_empty() {
        index += 1;
    }

    let mut variants = Vec::new();
    while let Some(part) = parts.get(index).copied() {
        if !is_variant_subtag(part) {
            break;
        }
        variants.push(part.to_string());
        index += 1;
    }

    let mut extensions = Vec::new();
    let mut parse_error = false;
    while let Some(singleton) = parts.get(index).copied() {
        if singleton.eq_ignore_ascii_case("x") {
            break;
        }
        if !is_extension_singleton(singleton) {
            break;
        }
        let singleton = singleton.as_bytes()[0].to_ascii_lowercase() as char;
        index += 1;
        let start = index;
        let mut subtags = Vec::new();
        while let Some(part) = parts.get(index).copied() {
            if !is_extension_subtag(part) {
                break;
            }
            subtags.push(part.to_string());
            index += 1;
        }
        if index == start {
            parse_error = true;
            break;
        }
        extensions.push(ParsedExtension { singleton, subtags });
    }

    let private_use = if parse_error {
        Vec::new()
    } else {
        parse_private_use_subtags(&parts, index, &mut parse_error)
    };
    LanguageTagParts {
        language,
        extlangs,
        script,
        region,
        variants,
        extensions,
        private_use,
    }
}

/// The pinned JDK `LanguageTag.LEGACY` table is a parser input mapping, not a
/// later canonicalization heuristic. Keeping it at the parser boundary makes
/// the remainder of the pipeline operate on the same mapped tag as JDK25.
fn legacy_language_tag(input: &str) -> Option<&'static str> {
    const LEGACY: &[(&str, &str)] = &[
        ("art-lojban", "jbo"),
        ("cel-gaulish", "xtg-x-cel-gaulish"),
        ("en-gb-oed", "en-GB-x-oed"),
        ("i-ami", "ami"),
        ("i-bnn", "bnn"),
        ("i-default", "en-x-i-default"),
        ("i-enochian", "und-x-i-enochian"),
        ("i-hak", "hak"),
        ("i-klingon", "tlh"),
        ("i-lux", "lb"),
        ("i-mingo", "see-x-i-mingo"),
        ("i-navajo", "nv"),
        ("i-pwn", "pwn"),
        ("i-tao", "tao"),
        ("i-tay", "tay"),
        ("i-tsu", "tsu"),
        ("no-bok", "nb"),
        ("no-nyn", "nn"),
        ("sgn-be-fr", "sfb"),
        ("sgn-be-nl", "vgt"),
        ("sgn-ch-de", "sgg"),
        ("zh-guoyu", "cmn"),
        ("zh-hakka", "hak"),
        ("zh-min", "nan-x-zh-min"),
        ("zh-min-nan", "nan"),
        ("zh-xiang", "hsn"),
    ];
    LEGACY
        .iter()
        .find(|(legacy, _mapped)| ascii_tag_equals_ignore_case(legacy, input))
        .map(|(_legacy, mapped)| *mapped)
}

fn parse_private_use_subtags(
    parts: &[&str],
    mut index: usize,
    parse_error: &mut bool,
) -> Vec<String> {
    if !parts
        .get(index)
        .is_some_and(|part| part.eq_ignore_ascii_case("x"))
    {
        return Vec::new();
    }
    index += 1;
    let start = index;
    let mut private_use = Vec::new();
    while let Some(part) = parts.get(index).copied() {
        if !is_private_use_subtag(part) {
            break;
        }
        private_use.push(part.to_string());
        index += 1;
    }
    if index == start {
        *parse_error = true;
        Vec::new()
    } else {
        private_use
    }
}

fn build_locale_identity(parts: &LanguageTagParts) -> LocaleIdentity {
    let language = parts
        .extlangs
        .first()
        .map(|value| canonical_language(value))
        .unwrap_or_else(|| {
            if parts.language.eq_ignore_ascii_case("und") {
                String::new()
            } else {
                canonical_language(&parts.language)
            }
        });
    let mut extensions = build_locale_extensions(parts);
    let mut variant = parts.variants.clone();
    extract_private_use_variant(&mut variant, &mut extensions.private_use);
    for value in &mut extensions.private_use {
        *value = value.to_ascii_lowercase();
    }
    LocaleIdentity {
        language,
        script: titlecase_ascii(&parts.script),
        region: parts.region.to_ascii_uppercase(),
        variant,
        extensions,
    }
}

fn build_locale_extensions(parts: &LanguageTagParts) -> LocaleExtensionsModel {
    let mut regular = Vec::new();
    let mut unicode_attributes = Vec::new();
    let mut unicode_keywords = Vec::new();
    let mut private_use = parts.private_use.clone();
    let mut seen = Vec::new();
    for extension in &parts.extensions {
        if seen.contains(&extension.singleton) {
            continue;
        }
        seen.push(extension.singleton);
        if extension.singleton == 'u' {
            (unicode_attributes, unicode_keywords) =
                canonicalize_unicode_extension(&extension.subtags);
        } else {
            regular.push(ParsedExtension {
                singleton: extension.singleton,
                subtags: extension
                    .subtags
                    .iter()
                    .map(|value| value.to_ascii_lowercase())
                    .collect(),
            });
        }
    }
    regular.sort_by_key(|extension| extension.singleton);
    LocaleExtensionsModel {
        regular,
        unicode_attributes,
        unicode_keywords,
        private_use: std::mem::take(&mut private_use),
    }
}

fn extract_private_use_variant(variant: &mut Vec<String>, private_use: &mut Vec<String>) {
    let Some(index) = private_use
        .iter()
        .position(|value| value.eq_ignore_ascii_case("lvariant"))
    else {
        return;
    };
    if index + 1 >= private_use.len() {
        return;
    }
    variant.extend(private_use[index + 1..].iter().cloned());
    private_use.truncate(index);
}

fn canonicalize_unicode_extension(values: &[String]) -> (Vec<String>, Vec<(String, String)>) {
    let mut index = 0;
    let mut attributes = Vec::new();
    while let Some(value) = values.get(index) {
        if !is_unicode_attribute(value) {
            break;
        }
        attributes.push(value.to_ascii_lowercase());
        index += 1;
    }
    attributes.sort();
    attributes.dedup();

    let mut keywords = Vec::new();
    while let Some(raw_key) = values.get(index) {
        if !is_unicode_key(raw_key) {
            break;
        }
        let key = raw_key.to_ascii_lowercase();
        index += 1;
        let start = index;
        while let Some(value) = values.get(index) {
            if !is_unicode_type_subtag(value) {
                break;
            }
            index += 1;
        }
        if !keywords.iter().any(|(existing, _)| *existing == key) {
            keywords.push((key, values[start..index].join("-").to_ascii_lowercase()));
        }
    }
    keywords.sort_by(|left, right| left.0.cmp(&right.0));
    (attributes, keywords)
}

fn apply_java_legacy_compatibility(base: &LocaleIdentity) -> LocaleExtensionsModel {
    // These are the two exact BaseLocale compatibility extensions used by
    // java.util.Locale for the historical Japanese and Thai variants. They
    // are serialized compatibility data, not display-name special cases.
    if !base.extensions.is_empty() {
        return base.extensions.clone();
    }
    let compatibility = match (
        base.language.as_str(),
        base.region.as_str(),
        base.variant.as_slice(),
    ) {
        ("ja", "JP", [variant]) if variant == "JP" => Some(("ca", "japanese")),
        ("th", "TH", [variant]) if variant == "TH" => Some(("nu", "thai")),
        _ => None,
    };
    let Some((key, value)) = compatibility else {
        return base.extensions.clone();
    };
    LocaleExtensionsModel {
        regular: Vec::new(),
        unicode_attributes: Vec::new(),
        unicode_keywords: vec![(key.to_string(), value.to_string())],
        private_use: Vec::new(),
    }
}

fn serialize_locale_tag(base: &LocaleIdentity) -> String {
    let legacy_no_ny = base.language == "no"
        && base.region == "NO"
        && base.variant.len() == 1
        && base.variant[0] == "NY";
    let output_language = if legacy_no_ny { "nn" } else { &base.language };
    let extensions = apply_java_legacy_compatibility(base);
    let mut parts = Vec::new();
    if !output_language.is_empty() {
        parts.push(output_language.to_ascii_lowercase());
    }
    if !base.script.is_empty() {
        parts.push(titlecase_ascii(&base.script));
    }
    if !base.region.is_empty() {
        parts.push(base.region.to_ascii_uppercase());
    }

    let mut invalid_variants = Vec::new();
    if !legacy_no_ny {
        for (index, variant) in base.variant.iter().enumerate() {
            if is_variant_subtag(variant) {
                parts.push(variant.clone());
            } else {
                invalid_variants.extend(base.variant.iter().skip(index).cloned());
                break;
            }
        }
    }

    let has_base_subtag = !base.script.is_empty()
        || !base.region.is_empty()
        || (!legacy_no_ny && base.variant.iter().any(|value| is_variant_subtag(value)))
        || !extensions.regular.is_empty()
        || !extensions.unicode_attributes.is_empty()
        || !extensions.unicode_keywords.is_empty();
    let mut serialized_extensions = extensions.regular.clone();
    if !extensions.unicode_attributes.is_empty() || !extensions.unicode_keywords.is_empty() {
        let mut subtags = extensions.unicode_attributes.clone();
        subtags.extend(extensions.unicode_keywords.iter().flat_map(|(key, value)| {
            std::iter::once(key.clone()).chain((!value.is_empty()).then(|| value.clone()))
        }));
        serialized_extensions.push(ParsedExtension {
            singleton: 'u',
            subtags,
        });
    }
    serialized_extensions.sort_by_key(|extension| extension.singleton);
    for extension in serialized_extensions {
        parts.push(format!(
            "{}-{}",
            extension.singleton,
            extension.subtags.join("-")
        ));
    }

    let mut private_use = extensions.private_use.clone();
    let existing_private_use_len = private_use.len();
    if !legacy_no_ny && !invalid_variants.is_empty() {
        private_use.push("lvariant".to_string());
        private_use.extend(invalid_variants);
    }
    if !private_use.is_empty() {
        parts.push(format!(
            "x-{}",
            private_use
                .iter()
                .enumerate()
                .map(|(index, value)| {
                    if index >= existing_private_use_len {
                        value.clone()
                    } else {
                        value.to_ascii_lowercase()
                    }
                })
                .collect::<Vec<_>>()
                .join("-")
        ));
    }
    if base.language.is_empty() && (has_base_subtag || private_use.is_empty()) {
        parts.insert(0, "und".to_string());
    }
    parts.join("-")
}

fn build_resolved_locale(base: LocaleIdentity) -> ResolvedLocale {
    let tag = serialize_locale_tag(&base);
    let english = LocaleIdentity {
        language: "en".to_string(),
        script: String::new(),
        region: String::new(),
        variant: Vec::new(),
        extensions: LocaleExtensionsModel {
            regular: Vec::new(),
            unicode_attributes: Vec::new(),
            unicode_keywords: Vec::new(),
            private_use: Vec::new(),
        },
    };
    ResolvedLocale {
        code: base.language.clone(),
        country_code: (!base.region.is_empty()).then(|| base.region.clone()),
        display_name: build_display_name(&base, &english),
        localized_name: build_display_name(&base, &base),
        localized_country_name: display_country_name(&base, &base),
        short_tag: base.language.clone(),
        tag,
        base,
    }
}

fn resolved_from_name_record(record: &LocaleRecord) -> ResolvedLocale {
    let base = LocaleIdentity {
        language: record.code.to_string(),
        script: record.script.to_string(),
        region: record.country_code.to_string(),
        variant: record
            .variant
            .split('_')
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect(),
        extensions: LocaleExtensionsModel {
            regular: Vec::new(),
            unicode_attributes: Vec::new(),
            unicode_keywords: Vec::new(),
            private_use: Vec::new(),
        },
    };
    ResolvedLocale {
        base,
        tag: record.tag.to_string(),
        code: record.code.to_string(),
        country_code: (!record.country_code.is_empty()).then(|| record.country_code.to_string()),
        display_name: record.display_name.to_string(),
        localized_name: record.localized_name.to_string(),
        localized_country_name: (!record.localized_country_name.is_empty())
            .then(|| record.localized_country_name.to_string()),
        short_tag: record.code.to_string(),
    }
}

fn parse_language_tag(input: &str) -> Option<ResolvedLocale> {
    Some(build_resolved_locale(build_locale_identity(
        &parse_jdk_language_tag(input),
    )))
}

fn find_tag_record(tag: &str) -> Option<&'static LocaleRecord> {
    LOCALE_TAG_RECORDS
        .binary_search_by(|record| record.tag.cmp(tag))
        .ok()
        .map(|index| &LOCALE_TAG_RECORDS[index])
}

fn record_base_matches(record: &LocaleRecord, base: &LocaleIdentity) -> bool {
    record.code == base.language
        && record.script == base.script
        && record.country_code == base.region
        && record
            .variant
            .split('_')
            .filter(|value| !value.is_empty())
            .eq(base.variant.iter().map(String::as_str))
        && base.extensions.is_empty()
}

fn resolve_detailed(identifier: &str) -> Option<ResolvedLocale> {
    if let Some(record) = LOCALE_NAME_RECORDS
        .iter()
        .find(|record| string_equals_ignore_case(record.display_name, identifier))
    {
        return Some(resolved_from_name_record(record));
    }
    let resolved = parse_language_tag(identifier)?;
    if let Some(record) = find_tag_record(&resolved.tag) {
        if record_base_matches(record, &resolved.base) {
            return Some(ResolvedLocale {
                base: resolved.base,
                tag: resolved.tag,
                code: record.code.to_string(),
                country_code: (!record.country_code.is_empty())
                    .then(|| record.country_code.to_string()),
                display_name: record.display_name.to_string(),
                localized_name: record.localized_name.to_string(),
                localized_country_name: (!record.localized_country_name.is_empty())
                    .then(|| record.localized_country_name.to_string()),
                short_tag: record.code.to_string(),
            });
        }
    }
    Some(resolved)
}

fn is_language_subtag(value: &str) -> bool {
    (2..=8).contains(&value.len()) && is_alpha_subtag(value)
}

fn is_extlang_subtag(value: &str) -> bool {
    value.len() == 3 && is_alpha_subtag(value)
}

fn is_script_subtag(value: &str) -> bool {
    value.len() == 4 && is_alpha_subtag(value)
}

fn is_region_subtag(value: &str) -> bool {
    (value.len() == 2 && is_alpha_subtag(value))
        || (value.len() == 3 && value.bytes().all(|byte| byte.is_ascii_digit()))
}

fn is_extension_singleton(value: &str) -> bool {
    value.len() == 1 && is_alpha_subtag(value) && !value.eq_ignore_ascii_case("x")
}

fn is_extension_subtag(value: &str) -> bool {
    (2..=8).contains(&value.len()) && is_alphanumeric_subtag(value)
}

fn is_private_use_subtag(value: &str) -> bool {
    (1..=8).contains(&value.len()) && is_alphanumeric_subtag(value)
}

fn is_alpha_subtag(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_alphabetic())
}

fn is_alphanumeric_subtag(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

fn is_variant_subtag(value: &str) -> bool {
    (5..=8).contains(&value.len()) && is_alphanumeric_subtag(value)
        || value.len() == 4
            && value.as_bytes()[0].is_ascii_digit()
            && value.as_bytes()[1..]
                .iter()
                .all(|byte| byte.is_ascii_alphanumeric())
}

fn is_unicode_attribute(value: &str) -> bool {
    (3..=8).contains(&value.len()) && is_alphanumeric_subtag(value)
}

fn is_unicode_key(value: &str) -> bool {
    value.len() == 2 && is_alphanumeric_subtag(value)
}

fn is_unicode_type_subtag(value: &str) -> bool {
    (3..=8).contains(&value.len()) && is_alphanumeric_subtag(value)
}

fn titlecase_ascii(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    first.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase()
}

fn canonical_language(language: &str) -> String {
    match language.to_ascii_lowercase().as_str() {
        "iw" => "he".to_string(),
        "in" => "id".to_string(),
        "ji" => "yi".to_string(),
        language => language.to_string(),
    }
}

fn build_display_name(base: &LocaleIdentity, display_locale: &LocaleIdentity) -> String {
    let language = if base.language.is_empty() {
        String::new()
    } else {
        display_data_value(display_locale, &base.language)
            .unwrap_or(&base.language)
            .to_string()
    };
    let script = if base.script.is_empty() {
        String::new()
    } else {
        display_data_value(display_locale, &base.script)
            .unwrap_or(&base.script)
            .to_string()
    };
    let country = display_country_name(base, display_locale).unwrap_or_default();
    let mut names = Vec::new();
    if !language.is_empty() {
        names.push(language);
    }
    if !script.is_empty() {
        names.push(script);
    }
    if !country.is_empty() {
        names.push(country);
    }
    let variants = base
        .variant
        .iter()
        .map(|variant| display_variant(display_locale, variant))
        .collect::<Vec<_>>();
    names.extend(variants);

    // Locale.getDisplayName returns a plain formatted variant list when no
    // language/script/country exists, and ignores Unicode extensions there.
    if names.is_empty() {
        return format_display_list(display_locale, &names);
    }
    names.extend(
        base.extensions
            .unicode_attributes
            .iter()
            .map(|attribute| display_unicode_attribute(display_locale, attribute)),
    );
    names.extend(
        base.extensions
            .unicode_keywords
            .iter()
            .map(|(key, value)| display_unicode_keyword(display_locale, key, value)),
    );
    let main = names.remove(0);
    format_display_name(display_locale, &main, &names)
}

fn display_country_name(base: &LocaleIdentity, display_locale: &LocaleIdentity) -> Option<String> {
    (!base.region.is_empty()).then(|| {
        display_data_value(display_locale, &base.region)
            .unwrap_or(&base.region)
            .to_string()
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CandidateLocale {
    language: String,
    script: String,
    region: String,
    variants: Vec<String>,
}

fn candidate_base_locale_tag(candidate: &CandidateLocale) -> String {
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

fn candidate_locales(base: &LocaleIdentity) -> Vec<CandidateLocale> {
    // ResourceBundle.Control receives the historical BaseLocale-like fields,
    // not Locale.toLanguageTag()'s compatibility serialization. In particular,
    // no/NO/NY remains the request identity while the language-tag output is
    // nn-NO.
    let language = base.language.as_str();
    let script = base.script.as_str();
    let region = base.region.as_str();
    let mut variants = base.variant.clone();
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

    candidates
}

#[cfg(test)]
fn display_candidate_profiles(base: &LocaleIdentity) -> Vec<String> {
    let mut profiles = Vec::new();
    for candidate in candidate_locales(base) {
        let profile = candidate_base_locale_tag(&candidate);
        if !profiles.contains(&profile) {
            profiles.push(profile);
        }
    }
    profiles
}

fn cldr_parent_locale(candidate: &CandidateLocale) -> Option<CandidateLocale> {
    let tag = candidate_base_locale_tag(candidate);
    if let Some((_, parent)) = CLDR_PARENT_LOCALES.iter().find(|(child, _)| *child == tag) {
        return parse_candidate(parent);
    }
    if candidate.region.is_empty() && !candidate.script.is_empty() {
        let likely_script = CLDR_LIKELY_SCRIPTS
            .iter()
            .find(|(language, _)| *language == candidate.language)
            .map(|(_, script)| *script);
        if likely_script.is_some_and(|script| script != candidate.script) {
            return Some(CandidateLocale {
                language: String::new(),
                script: String::new(),
                region: String::new(),
                variants: Vec::new(),
            });
        }
    }
    None
}

fn parse_candidate(tag: &str) -> Option<CandidateLocale> {
    if tag.is_empty() {
        return Some(CandidateLocale {
            language: String::new(),
            script: String::new(),
            region: String::new(),
            variants: Vec::new(),
        });
    }
    let mut parts = tag.split('-');
    let language = parts.next()?.to_string();
    let script = parts
        .next()
        .filter(|part| part.len() == 4)
        .map(str::to_string)
        .unwrap_or_default();
    let region = if script.is_empty() {
        parts
            .next()
            .filter(|part| (part.len() == 2 && is_alpha_subtag(part)) || part.len() == 3)
            .map(str::to_string)
            .unwrap_or_default()
    } else {
        parts
            .next()
            .filter(|part| (part.len() == 2 && is_alpha_subtag(part)) || part.len() == 3)
            .map(str::to_string)
            .unwrap_or_default()
    };
    let variants = parts.map(str::to_string).collect();
    Some(CandidateLocale {
        language,
        script,
        region,
        variants,
    })
}

fn cldr_candidate_locales(base: &LocaleIdentity) -> Vec<CandidateLocale> {
    let initial = candidate_base_locale_tag(&CandidateLocale {
        language: base.language.clone(),
        script: base.script.clone(),
        region: base.region.clone(),
        variants: base.variant.clone(),
    });
    let aliased = CLDR_LANGUAGE_ALIASES
        .iter()
        .find(|(alias, _)| *alias == initial)
        .and_then(|(_, target)| parse_candidate(target));
    let mut base = if let Some(aliased) = aliased {
        default_candidate_locales(
            &aliased.language,
            &aliased.script,
            &aliased.region,
            &aliased.variants,
        )
    } else {
        candidate_locales(base)
    };

    // CLDR's parent-locale graph is applied after ResourceBundle.Control's
    // ordinary candidate construction. This is provider routing, not the
    // public BaseLocale candidate identity exposed by the oracle test.
    for index in 0..base.len().saturating_sub(1) {
        let Some(parent) = cldr_parent_locale(&base[index]) else {
            continue;
        };
        if parent == base[index + 1] {
            continue;
        }
        let mut replacement = base[..=index].to_vec();
        if parent.language.is_empty()
            && parent.script.is_empty()
            && parent.region.is_empty()
            && parent.variants.is_empty()
        {
            replacement.push(parent);
        } else if parent.language == "no" {
            replacement.push(parent);
            replacement.push(CandidateLocale {
                language: String::new(),
                script: String::new(),
                region: String::new(),
                variants: Vec::new(),
            });
        } else {
            replacement.extend(cldr_candidate_locales_for_components(
                &parent.language,
                &parent.script,
                &parent.region,
                &parent.variants,
            ));
        }
        base = replacement;
        break;
    }
    base
}

fn cldr_candidate_locales_for_components(
    language: &str,
    script: &str,
    region: &str,
    variants: &[String],
) -> Vec<CandidateLocale> {
    let identity = LocaleIdentity {
        language: language.to_string(),
        script: script.to_string(),
        region: region.to_string(),
        variant: variants.to_vec(),
        extensions: LocaleExtensionsModel {
            regular: Vec::new(),
            unicode_attributes: Vec::new(),
            unicode_keywords: Vec::new(),
            private_use: Vec::new(),
        },
    };
    cldr_candidate_locales(&identity)
}

fn display_data_value(display_locale: &LocaleIdentity, key: &str) -> Option<&'static str> {
    let snapshot = DisplaySnapshot::parse(LOCALE_DISPLAY_DATA)?;
    cldr_candidate_locales(display_locale)
        .iter()
        .find_map(|candidate| {
            let profile = candidate_base_locale_tag(candidate);
            snapshot
                .profile_id(&profile)
                .and_then(|profile_id| snapshot.resolve_profile(profile_id, key))
        })
}

fn display_variant(display_locale: &LocaleIdentity, variant: &str) -> String {
    let key = format!("%%{variant}");
    display_data_value(display_locale, &key)
        .unwrap_or(variant)
        .to_string()
}

fn display_unicode_attribute(display_locale: &LocaleIdentity, attribute: &str) -> String {
    let key = format!("key.{attribute}");
    display_data_value(display_locale, &key)
        .unwrap_or(attribute)
        .to_string()
}

fn display_unicode_keyword(display_locale: &LocaleIdentity, key: &str, value: &str) -> String {
    let key_name = format!("key.{key}");
    let type_name = format!("type.{key}.{value}");
    if let Some(display_type) =
        display_data_value(display_locale, &type_name).filter(|name| *name != value)
    {
        return display_type.to_string();
    }
    // Locale.getDisplayName has three provider fallbacks for Unicode keyword
    // types that are not LocaleNames `type.*` records. These branches select
    // generated data families, never case-specific display literals.
    let display_type = if key == "cu" {
        let currency_key = format!("currency.{value}");
        display_data_value(display_locale, &currency_key).unwrap_or(value)
    } else if key == "rg"
        && value.len() == 6
        && value.as_bytes()[..2].iter().all(u8::is_ascii_alphabetic)
        && value[2..].eq_ignore_ascii_case("zzzz")
    {
        let region = value[..2].to_ascii_uppercase();
        display_data_value(display_locale, &region).unwrap_or(value)
    } else if key == "tz" {
        let timezone_key = format!("timezone.{value}");
        display_data_value(display_locale, &timezone_key).unwrap_or(value)
    } else {
        value
    };
    let display_key = display_data_value(display_locale, &key_name).unwrap_or(key);
    let pattern = display_data_value(display_locale, "ListKeyTypePattern").unwrap_or("{0}: {1}");
    format_message_pair(pattern, display_key, display_type)
}

fn format_display_list(display_locale: &LocaleIdentity, values: &[String]) -> String {
    if values.is_empty() {
        return String::new();
    }
    let pattern =
        display_data_value(display_locale, "ListCompositionPattern").unwrap_or("{0}, {1}");
    values.iter().fold(String::new(), |prefix, value| {
        if prefix.is_empty() || value.is_empty() {
            if prefix.is_empty() {
                value.clone()
            } else {
                prefix
            }
        } else {
            format_message_pair(pattern, &prefix, value)
        }
    })
}

fn format_display_name(
    display_locale: &LocaleIdentity,
    main: &str,
    qualifiers: &[String],
) -> String {
    if qualifiers.is_empty() {
        return main.to_string();
    }
    let list = format_display_list(display_locale, qualifiers);
    let pattern = display_data_value(display_locale, "DisplayNamePattern");
    let Some(choice) = pattern
        .and_then(|value| value.strip_prefix("{0,choice,"))
        .and_then(|value| value.strip_suffix('}'))
    else {
        return format!("{main} ({list})");
    };
    let branch = choice
        .split('|')
        .find_map(|branch| branch.strip_prefix("2#"))
        .unwrap_or("{1} ({2})");
    format_display_choice(branch, main, &list)
}

fn format_message_pair(pattern: &str, first: &str, second: &str) -> String {
    pattern.replace("{0}", first).replace("{1}", second)
}

fn format_display_choice(pattern: &str, main: &str, qualifiers: &str) -> String {
    pattern.replace("{1}", main).replace("{2}", qualifiers)
}

#[cfg(test)]
mod tests {
    use super::{
        ascii_tag_equals_ignore_case, display_candidate_profiles, parse_language_tag, resolve,
        resolve_detailed, string_equals_ignore_case,
    };
    use super::{
        read_u32, DisplaySnapshot, LOCALE_AVAILABLE_ORDER_MANIFEST_SHA256,
        LOCALE_AVAILABLE_RECORD_COUNT, LOCALE_DATASET_SOURCE_SHA256, LOCALE_DATASET_VERSION,
        LOCALE_DISPLAY_COMPACT_FORMAT_VERSION, LOCALE_DISPLAY_COMPACT_RECORD_COUNT,
        LOCALE_DISPLAY_COMPACT_SHA256, LOCALE_DISPLAY_COMPACT_SNAPSHOT_BYTES, LOCALE_DISPLAY_DATA,
        LOCALE_DISPLAY_GENERATED_SOURCE_BYTES, LOCALE_DISPLAY_KEY_COUNT, LOCALE_DISPLAY_MAGIC,
        LOCALE_DISPLAY_MAX_COMPACT_SNAPSHOT_BYTES, LOCALE_DISPLAY_MAX_GENERATED_SOURCE_BYTES,
        LOCALE_DISPLAY_NUMERIC_INDEX_BYTES, LOCALE_DISPLAY_ORACLE_RECORD_COUNT,
        LOCALE_DISPLAY_PROFILE_COUNT, LOCALE_DISPLAY_RAW_STRING_POOL_BYTES,
        LOCALE_DISPLAY_RECORD_COUNT, LOCALE_DISPLAY_SOURCE_SHA256, LOCALE_DISPLAY_VALUE_COUNT,
        LOCALE_NAME_COLLISIONS, LOCALE_NAME_COLLISION_COUNT, LOCALE_NAME_COLLISION_MEMBER_TAGS,
        LOCALE_NAME_RECORDS, LOCALE_PUBLIC_ORACLE_OUTPUT_SHA256, LOCALE_PUBLIC_ORACLE_RECORD_COUNT,
        LOCALE_TAG_RECORDS, LOCALE_TAG_RECORD_COUNT,
    };

    #[test]
    fn dataset_identity_and_indexes_are_guarded() {
        assert_eq!(LOCALE_AVAILABLE_RECORD_COUNT, LOCALE_NAME_RECORDS.len());
        assert_eq!(LOCALE_TAG_RECORD_COUNT, LOCALE_TAG_RECORDS.len());
        assert_eq!(LOCALE_TAG_RECORDS.len(), 1157);
        assert_eq!(LOCALE_NAME_RECORDS.len(), 1158);
        assert_eq!(LOCALE_TAG_RECORDS[0].tag, "af");
        assert_eq!(LOCALE_TAG_RECORDS.last().unwrap().tag, "zu-ZA");
        assert_eq!(LOCALE_DATASET_VERSION, "25.0.4.1+1");
        assert_eq!(
            LOCALE_DATASET_SOURCE_SHA256,
            "2dc572125ce0e50854fc3ec538acde3358c5b0320e13b501162411a34dc36105"
        );
        assert_eq!(
            LOCALE_AVAILABLE_ORDER_MANIFEST_SHA256,
            "c4dd6cd7e83919d7236d3040c1ddc60ca21ff92e179b19a7d7d10fda7f9a815e"
        );
        assert_eq!(LOCALE_DISPLAY_RECORD_COUNT, 453459);
        assert_eq!(
            LOCALE_DISPLAY_RECORD_COUNT,
            LOCALE_DISPLAY_ORACLE_RECORD_COUNT
        );
        assert_eq!(
            LOCALE_DISPLAY_SOURCE_SHA256,
            "96d43b0ff823a4505bdb69ddd80bfd3056867b2c7c0bc27b6a50fc822c116ab3"
        );
        assert_eq!(LOCALE_DISPLAY_COMPACT_RECORD_COUNT, 267017);
        assert_eq!(LOCALE_DISPLAY_PROFILE_COUNT, 320);
        assert_eq!(LOCALE_DISPLAY_KEY_COUNT, 2525);
        assert_eq!(LOCALE_DISPLAY_VALUE_COUNT, 178930);
        assert_eq!(LOCALE_DISPLAY_RAW_STRING_POOL_BYTES, 3682380);
        assert_eq!(LOCALE_DISPLAY_NUMERIC_INDEX_BYTES, 2140296);
        assert_eq!(
            LOCALE_DISPLAY_COMPACT_SNAPSHOT_BYTES,
            LOCALE_DISPLAY_DATA.len()
        );
        assert_eq!(LOCALE_DISPLAY_COMPACT_FORMAT_VERSION, 1);
        assert_eq!(LOCALE_DISPLAY_COMPACT_SHA256.len(), 64);
        let generated_source = include_str!("locale_data.rs");
        assert_eq!(
            generated_source.len(),
            LOCALE_DISPLAY_GENERATED_SOURCE_BYTES
        );
        assert!(generated_source.len() < LOCALE_DISPLAY_MAX_GENERATED_SOURCE_BYTES);
        assert!(generated_source.lines().count() < 100_000);
        assert!(LOCALE_DISPLAY_DATA.len() <= LOCALE_DISPLAY_MAX_COMPACT_SNAPSHOT_BYTES);
        assert!(LOCALE_TAG_RECORDS
            .windows(2)
            .all(|records| records[0].tag < records[1].tag));
        assert_eq!(LOCALE_NAME_COLLISION_COUNT, LOCALE_NAME_COLLISIONS.len());
        assert_name_collision_audit();
    }

    #[test]
    fn available_name_collision_audit_preserves_raw_first_winner() {
        assert_eq!(LOCALE_NAME_COLLISION_COUNT, 0);
        assert!(LOCALE_NAME_COLLISION_MEMBER_TAGS.is_empty());
        assert!(LOCALE_NAME_COLLISIONS.is_empty());
        assert_name_collision_audit();
    }

    fn assert_name_collision_audit() {
        let mut discovered = Vec::new();
        for (index, record) in LOCALE_NAME_RECORDS.iter().enumerate() {
            let first_index = LOCALE_NAME_RECORDS
                .iter()
                .position(|candidate| {
                    string_equals_ignore_case(candidate.display_name, record.display_name)
                })
                .expect("record must be present in its own name class");
            if first_index != index {
                continue;
            }
            let members: Vec<_> = LOCALE_NAME_RECORDS
                .iter()
                .filter(|candidate| {
                    string_equals_ignore_case(candidate.display_name, record.display_name)
                })
                .map(|candidate| candidate.tag)
                .collect();
            if members.len() > 1 {
                discovered.push((record.display_name, members));
            }
        }
        assert_eq!(discovered.len(), LOCALE_NAME_COLLISIONS.len());
        for (display_name, members) in discovered {
            let collision = LOCALE_NAME_COLLISIONS
                .iter()
                .find(|collision| string_equals_ignore_case(collision.display_name, display_name))
                .expect("every runtime collision must be in the generated audit");
            assert_eq!(collision.member_count, members.len());
            let audited_members = &LOCALE_NAME_COLLISION_MEMBER_TAGS
                [collision.member_start..collision.member_start + collision.member_count];
            assert_eq!(audited_members, members.as_slice());
            assert_eq!(collision.winner_tag, members[0]);
            assert_eq!(resolve(display_name).unwrap().tag, members[0]);
        }
    }

    #[test]
    fn compact_display_snapshot_has_valid_integrity_and_fingerprint() {
        let snapshot = DisplaySnapshot::parse(LOCALE_DISPLAY_DATA)
            .expect("checked-in display snapshot should have a valid header");
        snapshot
            .validate()
            .expect("checked-in display snapshot should satisfy every index invariant");
        assert_eq!(
            LOCALE_DISPLAY_DATA.get(..4),
            Some(LOCALE_DISPLAY_MAGIC.as_slice())
        );
        assert_eq!(
            read_u32(LOCALE_DISPLAY_DATA, 4),
            Some(LOCALE_DISPLAY_COMPACT_FORMAT_VERSION)
        );
        assert_eq!(snapshot.profile_count, LOCALE_DISPLAY_PROFILE_COUNT);
        assert_eq!(snapshot.key_count, LOCALE_DISPLAY_KEY_COUNT);
        assert_eq!(snapshot.value_count, LOCALE_DISPLAY_VALUE_COUNT);
        assert_eq!(snapshot.record_count, LOCALE_DISPLAY_COMPACT_RECORD_COUNT);
        assert_eq!(
            LOCALE_DISPLAY_DATA.len(),
            LOCALE_DISPLAY_COMPACT_SNAPSHOT_BYTES
        );
        assert_eq!(
            sha256_hex(LOCALE_DISPLAY_DATA),
            LOCALE_DISPLAY_COMPACT_SHA256
        );
    }

    fn sha256_hex(input: &[u8]) -> String {
        const ROUND_CONSTANTS: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
            0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
            0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
            0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
            0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
            0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
            0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
            0xc67178f2,
        ];
        let mut padded = input.to_vec();
        padded.push(0x80);
        while !(padded.len() + 8).is_multiple_of(64) {
            padded.push(0);
        }
        padded.extend_from_slice(&((input.len() as u64) * 8).to_be_bytes());
        let mut hash: [u32; 8] = [
            0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
            0x5be0cd19,
        ];
        let (chunks, remainder) = padded.as_chunks::<64>();
        debug_assert!(remainder.is_empty());
        for chunk in chunks {
            let mut words = [0u32; 64];
            for (index, word) in words.iter_mut().enumerate().take(16) {
                let start = index * 4;
                *word = u32::from_be_bytes([
                    chunk[start],
                    chunk[start + 1],
                    chunk[start + 2],
                    chunk[start + 3],
                ]);
            }
            for index in 16..64 {
                let s0 = words[index - 15].rotate_right(7)
                    ^ words[index - 15].rotate_right(18)
                    ^ (words[index - 15] >> 3);
                let s1 = words[index - 2].rotate_right(17)
                    ^ words[index - 2].rotate_right(19)
                    ^ (words[index - 2] >> 10);
                words[index] = words[index - 16]
                    .wrapping_add(s0)
                    .wrapping_add(words[index - 7])
                    .wrapping_add(s1);
            }
            let mut working = hash;
            for index in 0..64 {
                let sum1 = working[4].rotate_right(6)
                    ^ working[4].rotate_right(11)
                    ^ working[4].rotate_right(25);
                let choice = (working[4] & working[5]) ^ ((!working[4]) & working[6]);
                let temp1 = working[7]
                    .wrapping_add(sum1)
                    .wrapping_add(choice)
                    .wrapping_add(ROUND_CONSTANTS[index])
                    .wrapping_add(words[index]);
                let sum0 = working[0].rotate_right(2)
                    ^ working[0].rotate_right(13)
                    ^ working[0].rotate_right(22);
                let majority = (working[0] & working[1])
                    ^ (working[0] & working[2])
                    ^ (working[1] & working[2]);
                let temp2 = sum0.wrapping_add(majority);
                working[7] = working[6];
                working[6] = working[5];
                working[5] = working[4];
                working[4] = working[3].wrapping_add(temp1);
                working[3] = working[2];
                working[2] = working[1];
                working[1] = working[0];
                working[0] = temp1.wrapping_add(temp2);
            }
            for index in 0..8 {
                hash[index] = hash[index].wrapping_add(working[index]);
            }
        }
        let mut result = String::with_capacity(64);
        use std::fmt::Write as _;
        for word in hash {
            assert!(write!(&mut result, "{word:08x}").is_ok());
        }
        result
    }

    #[test]
    fn every_reference_tag_has_canonical_snapshot_data() {
        for record in LOCALE_TAG_RECORDS {
            let parsed = parse_language_tag(record.tag).expect("reference tag should parse");
            assert_eq!(parsed.tag, record.tag);
            let name_collision = LOCALE_NAME_RECORDS
                .iter()
                .any(|name| string_equals_ignore_case(name.display_name, record.tag));
            if !name_collision {
                let resolved = resolve(record.tag).expect("reference tag should resolve");
                assert_eq!(resolved.tag, record.tag);
                assert_eq!(resolved.localized_name, record.localized_name);
            }
        }
    }

    #[test]
    fn canonical_tag_collision_and_name_records_match_the_reference() {
        let resolved = resolve("Norwegian Nynorsk (Norway)").expect("name should resolve");
        assert_eq!(resolved.tag, "nn-NO");
        assert_eq!(resolved.localized_name, "norsk nynorsk (Noreg)");
        assert_eq!(
            resolve("nN-nO").unwrap().localized_name,
            "norsk nynorsk (Noreg)"
        );
    }

    #[test]
    fn jdk25_locale_oracle_matches_candidate_graph_and_public_resolution() {
        let Ok(path) = std::env::var("ARKST_JDK25_LOCALE_ORACLE") else {
            return;
        };
        let oracle_bytes = std::fs::read(path).expect("read transient JDK25 locale oracle");
        assert_eq!(
            sha256_hex(&oracle_bytes),
            LOCALE_PUBLIC_ORACLE_OUTPUT_SHA256,
            "transient public oracle fingerprint changed"
        );
        let oracle = std::str::from_utf8(&oracle_bytes).expect("public oracle must be UTF-8");
        let mut checked = 0usize;
        let mut candidate_checked = 0usize;
        for line in oracle.lines() {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 11, "malformed locale oracle row: {line}");
            assert_eq!(fields[0], "locale");
            let request = fields[1];
            let path_kind = fields[2];
            let expected_tag = fields[3];
            let expected_code = fields[4];
            let expected_country_code = fields[5];
            let expected_display_name = fields[6];
            let expected_name = fields[7];
            let expected_localized_country_name = fields[8];
            let expected_short_tag = fields[9];
            if path_kind == "tag" {
                let expected_candidates = fields[10]
                    .split('|')
                    .map(|value| {
                        if value == "<root>" {
                            String::new()
                        } else {
                            value.to_string()
                        }
                    })
                    .collect::<Vec<_>>();
                let parsed = parse_language_tag(request)
                    .unwrap_or_else(|| panic!("oracle tag request should be accepted: {request}"));
                assert_eq!(
                    display_candidate_profiles(&parsed.base),
                    expected_candidates,
                    "candidate graph mismatch for {request}"
                );
                candidate_checked += 1;
            } else {
                assert_eq!(path_kind, "name", "unknown oracle path: {line}");
                assert!(
                    fields[10].is_empty(),
                    "name path must not expose candidates"
                );
            }
            let actual = resolve_detailed(request)
                .unwrap_or_else(|| panic!("oracle request should resolve: {request}"));
            assert_eq!(
                actual.tag, expected_tag,
                "canonical tag mismatch for {request}"
            );
            assert_eq!(actual.code, expected_code, "code mismatch for {request}");
            assert_eq!(
                actual.country_code.as_deref().unwrap_or(""),
                expected_country_code,
                "countryCode mismatch for {request}"
            );
            assert_eq!(
                actual.display_name, expected_display_name,
                "displayName mismatch for {request}"
            );
            assert_eq!(
                actual.localized_name, expected_name,
                "localized name mismatch for {request}"
            );
            assert_eq!(
                actual.localized_country_name.as_deref().unwrap_or(""),
                expected_localized_country_name,
                "localizedCountryName mismatch for {request}"
            );
            assert_eq!(
                actual.short_tag, expected_short_tag,
                "shortTag mismatch for {request}"
            );
            checked += 1;
        }
        assert_eq!(
            checked, LOCALE_PUBLIC_ORACLE_RECORD_COUNT,
            "JDK25 locale oracle row count changed"
        );
        assert_eq!(
            candidate_checked, 5_122,
            "JDK25 tag-path oracle row count changed"
        );
    }

    #[test]
    fn name_matching_reuses_pinned_jdk25_characterwise_case() {
        assert!(string_equals_ignore_case("English", "eNgLiSh"));
        assert!(string_equals_ignore_case("İ", "i"));
        assert!(!string_equals_ignore_case("É", "e\u{301}"));
    }

    #[test]
    fn canonicalizes_aliases_and_structured_tags() {
        assert_eq!(parse_language_tag("EN-us").unwrap().tag, "en-US");
        assert_eq!(parse_language_tag("iw").unwrap().tag, "he");
        for (input, expected) in [
            ("ar-aao", "aao"),
            ("es-abc", "abc"),
            ("de-foo", "foo"),
            ("abcd-efg", "efg"),
        ] {
            assert_eq!(parse_language_tag(input).unwrap().tag, expected);
        }
        assert_eq!(
            parse_language_tag("ar-aao-Latn-EG").unwrap().tag,
            "aao-Latn-EG"
        );
        assert_eq!(parse_language_tag("i-klingon").unwrap().tag, "tlh");
        assert_eq!(parse_language_tag("zh-cmn").unwrap().tag, "cmn");
        assert_eq!(parse_language_tag("zh-yue").unwrap().tag, "yue");
        assert_eq!(parse_language_tag("zh-guoyu").unwrap().tag, "cmn");
        assert_eq!(parse_language_tag("zh-hakka").unwrap().tag, "hak");
        assert_eq!(parse_language_tag("zh-min").unwrap().tag, "nan-x-zh-min");
        assert_eq!(
            parse_language_tag("no-NO-x-lvariant-NY").unwrap().tag,
            "nn-NO"
        );
        assert_eq!(
            parse_language_tag("NO-no-x-LVARIANT-ny").unwrap().tag,
            "no-NO-x-lvariant-ny"
        );
        assert_eq!(
            parse_language_tag("no-Latn-NO-x-lvariant-NY").unwrap().tag,
            "nn-Latn-NO"
        );
        assert_eq!(
            parse_language_tag("no-NO-u-ca-gregory-x-lvariant-NY")
                .unwrap()
                .tag,
            "nn-NO-u-ca-gregory"
        );
        assert_eq!(
            parse_language_tag("no-NO-x-foo-lvariant-NY").unwrap().tag,
            "nn-NO-x-foo"
        );
        assert_eq!(
            parse_language_tag("no-NO-u-ca-gregory-x-foo-lvariant-NY")
                .unwrap()
                .tag,
            "nn-NO-u-ca-gregory-x-foo"
        );
        assert_eq!(parse_language_tag("no-NO-NY").unwrap().tag, "no-NO");
        assert_eq!(parse_language_tag("zh-min-nan").unwrap().tag, "nan");
        assert_eq!(parse_language_tag("zh-xiang").unwrap().tag, "hsn");
        assert_eq!(parse_language_tag("i-mingo").unwrap().tag, "see-x-i-mingo");
        assert_eq!(parse_language_tag("zh-hant-tw").unwrap().tag, "zh-Hant-TW");
        assert_eq!(parse_language_tag("de-DE-1996").unwrap().tag, "de-DE-1996");
    }

    #[test]
    fn legacy_no_ny_uses_the_derived_base_locale_for_localized_names() {
        for (input, expected_tag, expected_name) in [
            ("no-NO-x-lvariant-NY", "nn-NO", "norsk (Noreg, Nynorsk)"),
            (
                "no-NO-x-foo-lvariant-NY",
                "nn-NO-x-foo",
                "norsk (Noreg, Nynorsk)",
            ),
            (
                "no-NO-u-ca-gregory-x-lvariant-NY",
                "nn-NO-u-ca-gregory",
                "norsk (Noreg, Nynorsk, kalender: gregory)",
            ),
            (
                "no-NO-u-ca-gregory-x-foo-lvariant-NY",
                "nn-NO-u-ca-gregory-x-foo",
                "norsk (Noreg, Nynorsk, kalender: gregory)",
            ),
            (
                "no-Latn-NO-x-lvariant-NY",
                "nn-Latn-NO",
                "norsk (latinsk, Noreg, Nynorsk)",
            ),
            (
                "no-NO-x-lvariant-ny",
                "no-NO-x-lvariant-ny",
                "norsk (Norge, ny)",
            ),
        ] {
            let locale = resolve(input).expect("legacy Norwegian locale should resolve");
            assert_eq!(locale.tag, expected_tag, "input: {input}");
            assert_eq!(locale.localized_name, expected_name, "input: {input}");
        }
        let ordinary = resolve("no-NO-NY").expect("valid-prefix locale should resolve");
        assert_eq!(ordinary.tag, "no-NO");
        assert_eq!(ordinary.localized_name, "norsk (Norge)");
    }

    #[test]
    fn jdk25_chinese_provider_routing_and_display_names_are_regressed() {
        for (input, expected) in [
            ("zh-TW-u-ca-buddhist", "中文 (台灣，佛曆)"),
            ("zh-HK-u-ca-buddhist", "中文 (中國香港特別行政區，佛曆)"),
            ("zh-MO-u-ca-buddhist", "中文 (中國澳門特別行政區，佛曆)"),
            ("zh-CN-u-ca-buddhist", "中文 (中国，佛历)"),
            ("zh-SG-u-ca-buddhist", "中文 (新加坡，佛历)"),
            ("zh-Hant-TW-u-ca-buddhist", "中文 (繁體，台灣，佛曆)"),
            ("zh-Hans-CN-u-ca-buddhist", "中文 (简体，中国，佛历)"),
            ("zh-Hant", "中文 (繁體)"),
            ("zh-Hans", "中文 (简体)"),
        ] {
            assert_eq!(
                resolve(input)
                    .expect("Chinese locale should resolve")
                    .localized_name,
                expected,
                "input: {input}"
            );
        }
    }

    #[test]
    fn canonicalizes_private_use_variants_and_extension_order_like_java() {
        assert_eq!(
            parse_language_tag("en-US-x-lvariant-POSIX").unwrap().tag,
            "en-US-POSIX"
        );
        assert_eq!(
            parse_language_tag("en-x-abc-lvariant-Abcde-Defgh")
                .unwrap()
                .tag,
            "en-Abcde-Defgh-x-abc"
        );
        assert_eq!(
            parse_language_tag("en-x-lvariant-Abc").unwrap().tag,
            "en-x-lvariant-Abc"
        );
        assert_eq!(
            parse_language_tag("de-POSIX-x-URP-lvariant-Abcde-Defgh")
                .unwrap()
                .tag,
            "de-POSIX-Abcde-Defgh-x-urp"
        );
        assert_eq!(
            parse_language_tag("en-b-foo-a-bar").unwrap().tag,
            "en-a-bar-b-foo"
        );
        assert_eq!(
            parse_language_tag("en-u-nu-latn-ca-gregory").unwrap().tag,
            "en-u-ca-gregory-nu-latn"
        );
        assert_eq!(
            parse_language_tag("en-u-ca-gregory-ca-buddhist")
                .unwrap()
                .tag,
            "en-u-ca-gregory"
        );
        for (input, expected_tag, expected_name) in [
            (
                "ja-JP-x-lvariant-JP",
                "ja-JP-u-ca-japanese-x-lvariant-JP",
                "日本語 (日本、JP、和暦)",
            ),
            (
                "th-TH-x-lvariant-TH",
                "th-TH-u-nu-thai-x-lvariant-TH",
                "ไทย (ไทย, TH, ตัวเลขไทย)",
            ),
        ] {
            let locale = resolve(input).expect("legacy compatibility locale should resolve");
            assert_eq!(locale.tag, expected_tag, "input: {input}");
            assert_eq!(locale.localized_name, expected_name, "input: {input}");
        }
    }

    #[test]
    fn accepts_valid_unavailable_tags_without_host_lookup() {
        let locale = resolve("xx-YY").expect("JVM accepts a valid unavailable tag");
        assert_eq!(locale.tag, "xx-YY");
        assert_eq!(locale.localized_name, "xx (YY)");
        let locale = resolve("en-XX").expect("JVM accepts a valid unavailable tag");
        assert_eq!(locale.localized_name, "English (XX)");
        assert_eq!(resolve("ar-aao").unwrap().tag, "aao");
        assert_eq!(
            resolve("ar-aao-Latn-EG").unwrap().localized_name,
            "aao (Latin, Egypt)"
        );
        assert_eq!(resolve("not-a-locale").unwrap().localized_name, "not");
        assert_eq!(
            resolve("en-US-posix").unwrap().localized_name,
            "English (United States, posix)"
        );
        assert_eq!(
            resolve("qaa-Qaaa-001").unwrap().localized_name,
            "qaa (Qaaa, World)"
        );
        assert_eq!(
            resolve("en-u-nu-latn-ca-gregory").unwrap().localized_name,
            "English (Calendar: gregory, Western Digits)"
        );
        assert_eq!(
            resolve("en-u-ca-buddhist").unwrap().localized_name,
            "English (Buddhist Calendar)"
        );
        assert_eq!(
            resolve("fr-u-ca-buddhist").unwrap().localized_name,
            "français (calendrier bouddhiste)"
        );
        assert_eq!(
            resolve("en-u-ca-japanese").unwrap().localized_name,
            "English (Japanese Calendar)"
        );
        assert_eq!(
            resolve("en-u-nu-arab").unwrap().localized_name,
            "English (Arabic-Indic Digits)"
        );
        assert_eq!(
            resolve("en-u-co-phonebk").unwrap().localized_name,
            "English (Sort Order: phonebk)"
        );
        assert_eq!(
            resolve("en-u-cu-usd").unwrap().localized_name,
            "English (Currency: US Dollar)"
        );
        assert_eq!(
            resolve("en-u-rg-uszzzz").unwrap().localized_name,
            "English (Region For Supplemental Data: United States)"
        );
        assert_eq!(
            resolve("en-u-tz-usnyc").unwrap().localized_name,
            "English (Time Zone: Eastern Time)"
        );
        assert_eq!(
            resolve("fr-CA-u-cu-usd").unwrap().localized_name,
            "français (Canada, devise : dollar des États-Unis)"
        );
        assert_eq!(
            resolve("fr-CA-u-tz-usnyc").unwrap().localized_name,
            "français (Canada, fuseau horaire : heure de l’Est)"
        );
        assert_eq!(
            resolve("fr-u-ca-gregory").unwrap().localized_name,
            "français (calendrier\u{202f}: gregory)"
        );
        assert_eq!(
            resolve("de-POSIX-x-URP-lvariant-Abcde-Defgh")
                .unwrap()
                .localized_name,
            "Deutsch (Posix, Abcde, Defgh)"
        );
    }

    #[test]
    fn preserves_jdk_root_private_use_and_valid_prefix_results() {
        for (identifier, expected_tag, expected_name) in [
            ("", "und", ""),
            ("und", "und", ""),
            ("x-private", "x-private", ""),
            ("x-y-z-blork", "x-y-z-blork", ""),
            ("en_US", "und", ""),
            ("   ", "und", ""),
            ("en--US", "en", "English"),
            ("en-", "en", "English"),
        ] {
            let locale = resolve(identifier).expect("JDK returns a Locale for every input");
            assert_eq!(locale.tag, expected_tag, "identifier: {identifier:?}");
            assert_eq!(
                locale.localized_name, expected_name,
                "identifier: {identifier:?}"
            );
        }
        assert_eq!(
            resolve("ja-JP-x-lvariant-JP").unwrap().tag,
            "ja-JP-u-ca-japanese-x-lvariant-JP"
        );
        assert_eq!(resolve("no-NO-x-lvariant-NY").unwrap().tag, "nn-NO");
    }

    #[test]
    fn blank_language_keeps_empty_code_fields_and_jdk_tag_serialization() {
        for (input, expected_tag) in [
            ("", "und"),
            ("und", "und"),
            ("x-private", "x-private"),
            ("x-y-z-blork", "x-y-z-blork"),
            ("en_US", "und"),
            ("   ", "und"),
        ] {
            let parsed = parse_language_tag(input).expect("JDK returns a Locale for every input");
            assert!(parsed.base.language.is_empty(), "input: {input:?}");
            assert!(parsed.code.is_empty(), "input: {input:?}");
            assert!(parsed.short_tag.is_empty(), "input: {input:?}");
            assert!(parsed.display_name.is_empty(), "input: {input:?}");
            assert_eq!(parsed.tag, expected_tag, "input: {input:?}");
        }
        let parsed = parse_language_tag("en--US").expect("JDK keeps the valid prefix");
        assert_eq!(parsed.base.language, "en");
        assert_eq!(parsed.tag, "en");
    }

    #[test]
    fn language_tag_stage_keeps_private_use_outside_the_regular_grammar() {
        for input in [
            "x-abc",
            "x-Latn",
            "x-US",
            "x-001",
            "x-abc-def",
            "x-Latn-US",
            "x-US-abc",
        ] {
            let parts = super::parse_jdk_language_tag(input);
            assert!(parts.language.is_empty(), "input: {input}");
            assert!(parts.extlangs.is_empty(), "input: {input}");
            assert!(parts.script.is_empty(), "input: {input}");
            assert!(parts.region.is_empty(), "input: {input}");
            assert!(parts.variants.is_empty(), "input: {input}");
            assert!(parts.extensions.is_empty(), "input: {input}");
            assert!(!parts.private_use.is_empty(), "input: {input}");

            let base = super::build_locale_identity(&parts);
            assert!(base.language.is_empty(), "input: {input}");
            assert!(base.script.is_empty(), "input: {input}");
            assert!(base.region.is_empty(), "input: {input}");
            assert!(base.variant.is_empty(), "input: {input}");
            assert!(!base.extensions.private_use.is_empty(), "input: {input}");
            assert!(parse_language_tag(input)
                .expect("private-use-only input should resolve")
                .code
                .is_empty());
        }
    }

    #[test]
    fn builder_promotes_only_the_first_extlang_after_a_language() {
        let ordinary =
            super::build_locale_identity(&super::parse_jdk_language_tag("en-abc-def-ghi"));
        assert_eq!(ordinary.language, "abc");
        assert!(ordinary.script.is_empty());
        assert!(ordinary.region.is_empty());

        let private_only = super::build_locale_identity(&super::parse_jdk_language_tag("x-abc"));
        assert!(private_only.language.is_empty());
        assert!(private_only
            .extensions
            .private_use
            .contains(&"abc".to_string()));

        let und = super::build_locale_identity(&super::parse_jdk_language_tag("und-Latn-US"));
        assert!(und.language.is_empty());
        assert_eq!(und.script, "Latn");
        assert_eq!(und.region, "US");
    }

    #[test]
    fn pinned_jdk_legacy_map_is_applied_before_language_tag_building() {
        for (input, expected) in [
            ("art-lojban", "jbo"),
            ("cel-gaulish", "xtg-x-cel-gaulish"),
            ("en-GB-oed", "en-GB-x-oed"),
            ("i-ami", "ami"),
            ("i-bnn", "bnn"),
            ("i-default", "en-x-i-default"),
            ("i-enochian", "x-i-enochian"),
            ("i-hak", "hak"),
            ("i-klingon", "tlh"),
            ("i-lux", "lb"),
            ("i-mingo", "see-x-i-mingo"),
            ("i-navajo", "nv"),
            ("i-pwn", "pwn"),
            ("i-tao", "tao"),
            ("i-tay", "tay"),
            ("i-tsu", "tsu"),
            ("no-bok", "nb"),
            ("no-nyn", "nn"),
            ("sgn-BE-FR", "sfb"),
            ("sgn-BE-NL", "vgt"),
            ("sgn-CH-DE", "sgg"),
            ("zh-guoyu", "cmn"),
            ("zh-hakka", "hak"),
            ("zh-min", "nan-x-zh-min"),
            ("zh-min-nan", "nan"),
            ("zh-xiang", "hsn"),
        ] {
            assert_eq!(
                parse_language_tag(input).unwrap().tag,
                expected,
                "input: {input}"
            );
            assert_eq!(
                parse_language_tag(&input.to_ascii_uppercase()).unwrap().tag,
                expected
            );
        }
    }

    #[test]
    fn legacy_lookup_does_not_apply_unicode_case_equivalence() {
        assert!(string_equals_ignore_case("i-klingon", "i-Klingon"));
        assert!(string_equals_ignore_case("sgn-be-fr", "ſgn-BE-FR"));
        assert!(!ascii_tag_equals_ignore_case("i-klingon", "i-Klingon"));
        assert!(!ascii_tag_equals_ignore_case("sgn-be-fr", "ſgn-BE-FR"));
        assert_eq!(parse_language_tag("i-Klingon").unwrap().tag, "und");
        assert_eq!(parse_language_tag("ſgn-BE-FR").unwrap().tag, "und");
    }
}
