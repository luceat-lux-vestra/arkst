//! Deterministic locale records used by the `.doclang` slice.
//!
//! Quarkdown v2.5.1 delegates locale lookup to `java.util.Locale`. The
//! platform-neutral evaluator cannot use that host database, so the complete
//! available-locale snapshot from the pinned reference JDK is checked in as
//! generated Rust data. Runtime lookup has no JVM, OS, ICU, filesystem, or
//! network dependency.

use crate::unicode_case::{simple_lowercase, simple_uppercase};
use scribium_ir::IrDocumentLocale;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LocaleRecord {
    tag: &'static str,
    display_name: &'static str,
    localized_name: &'static str,
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
        let key_id = self.key_id(key)?;
        if let Some(value) = self.find_record(profile_id, key_id) {
            return Some(value);
        }
        let (start, end) = self.fallback_range(profile_id)?;
        (start..end).find_map(|index| {
            self.fallback_id(index)
                .and_then(|fallback_id| self.resolve_profile(fallback_id, key))
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
/// Name records retain the exact `getAvailableLocales()` order because
/// upstream `fromName` returns the first case-insensitive display-name match.
/// Canonical tag records are a separate deduplicated index because upstream
/// available locales contains one observable `nn-NO` collision while
/// `fromTag` constructs a canonical `Locale` (`nn_NO`) directly.
pub(crate) fn resolve(identifier: &str) -> Option<IrDocumentLocale> {
    if let Some(record) = LOCALE_NAME_RECORDS
        .iter()
        .find(|record| string_equals_ignore_case(record.display_name, identifier))
    {
        return Some(to_ir_locale(record));
    }

    let parsed = parse_language_tag(identifier)?;
    if parsed.display_base != parsed.canonical_base {
        // Locale.toLanguageTag() may serialize a compatibility-adjusted
        // language while getDisplayName() still uses the derived BaseLocale.
        return Some(to_fallback_locale(&parsed));
    }
    if let Some(record) = find_tag_record(&parsed.canonical) {
        return Some(to_ir_locale(record));
    }

    Some(to_fallback_locale(&parsed))
}

fn to_ir_locale(record: &LocaleRecord) -> IrDocumentLocale {
    IrDocumentLocale {
        tag: record.tag.to_string(),
        localized_name: record.localized_name.to_string(),
    }
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
    left_upper == right_upper
        || simple_lowercase(left_upper) == simple_lowercase(right_upper)
}

fn find_tag_record(tag: &str) -> Option<&'static LocaleRecord> {
    LOCALE_TAG_RECORDS
        .binary_search_by(|record| record.tag.cmp(tag))
        .ok()
        .map(|index| &LOCALE_TAG_RECORDS[index])
}

#[derive(Debug, PartialEq, Eq)]
struct ParsedLanguageTag {
    canonical: String,
    language: String,
    display_language: String,
    display_base: String,
    canonical_base: String,
    script: Option<String>,
    region: Option<String>,
    variants: Vec<String>,
    display_variants: Vec<String>,
    unicode_attributes: Vec<String>,
    unicode_keywords: Vec<(String, String)>,
}

fn parse_language_tag(input: &str) -> Option<ParsedLanguageTag> {
    if input.is_empty() {
        return None;
    }

    let parts: Vec<&str> = input.split('-').collect();

    if let Some(alias) = grandfathered_alias(&parts) {
        return parse_language_tag(alias);
    }

    let mut index = 0;
    let raw_language = *parts.first()?;
    if !is_alpha_subtag(raw_language) || !(2..=8).contains(&raw_language.len()) {
        return None;
    }
    let mut language = canonical_language(raw_language);
    index += 1;

    let mut extlangs = Vec::new();
    while extlangs.len() < 3 {
        let Some(part) = parts.get(index).copied() else {
            break;
        };
        if part.len() != 3 || !is_alpha_subtag(part) {
            break;
        }
        extlangs.push(part.to_ascii_lowercase());
        index += 1;
    }
    if let Some(first_extlang) = extlangs.first() {
        // InternalLocaleBuilder.setLanguageTag promotes the first extlang
        // unconditionally and discards the primary language and all remaining
        // extlangs. It does not consult the IANA registry.
        language = first_extlang.clone();
    }

    let script = parts
        .get(index)
        .copied()
        .filter(|part| part.len() == 4 && is_alpha_subtag(part))
        .map(titlecase_ascii);
    if script.is_some() {
        index += 1;
    }

    let region = parts
        .get(index)
        .copied()
        .filter(|part| {
            (part.len() == 2 && is_alpha_subtag(part))
                || (part.len() == 3 && part.bytes().all(|byte| byte.is_ascii_digit()))
        })
        .map(str::to_ascii_uppercase);
    if region.is_some() {
        index += 1;
    }

    let mut variants = Vec::new();
    while let Some(part) = parts.get(index).copied() {
        if is_variant_subtag(part) {
            variants.push(part.to_string());
            index += 1;
            continue;
        }
        break;
    }

    let mut extensions: Vec<(char, Vec<String>)> = Vec::new();
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
        let mut values = Vec::new();
        while let Some(part) = parts.get(index).copied() {
            if !(2..=8).contains(&part.len()) || !is_alphanumeric_subtag(part) {
                break;
            }
            values.push(part.to_ascii_lowercase());
            index += 1;
        }
        if index == start {
            // LanguageTag.parse records an incomplete extension as an error,
            // but Locale.forLanguageTag still retains the valid prefix parsed
            // before that extension and ignores the rest.
            parse_error = true;
            break;
        }
        if !extensions.iter().any(|(key, _)| *key == singleton) {
            extensions.push((singleton, values));
        }
    }

    let mut private_use = None;
    if !parse_error
        && parts
            .get(index)
            .is_some_and(|part| part.eq_ignore_ascii_case("x"))
    {
        index += 1;
        let start = index;
        let mut values = Vec::new();
        while let Some(part) = parts.get(index).copied() {
            if !(1..=8).contains(&part.len()) || !is_alphanumeric_subtag(part) {
                break;
            }
            values.push(part.to_string());
            index += 1;
        }
        if index != start {
            private_use = Some(values);
        }
    }

    let mut unicode_attributes = Vec::new();
    let mut unicode_keywords = Vec::new();
    let mut canonical_extensions = Vec::new();
    for (singleton, values) in extensions {
        if singleton == 'u' {
            let (attributes, keywords) = parse_unicode_extension(&values);
            unicode_attributes = attributes;
            unicode_keywords = keywords;
        } else {
            canonical_extensions.push((singleton, values.join("-")));
        }
    }

    let mut private_use_extension = None;
    if let Some(values) = private_use {
        let lvariant_index = values
            .iter()
            .position(|value| value.eq_ignore_ascii_case("lvariant"));
        if let Some(lvariant_index) = lvariant_index.filter(|index| *index + 1 < values.len()) {
            let private_variant_values = &values[lvariant_index + 1..];
            let valid_count = private_variant_values
                .iter()
                .take_while(|value| is_variant_subtag(value))
                .count();
            // InternalLocaleBuilder stores the entire suffix in the Java
            // variant field. Locale.toLanguageTag later moves only the
            // ill-formed tail back under x-lvariant.
            variants.extend(private_variant_values.iter().cloned());
            let invalid_private_variants = &private_variant_values[valid_count..];
            let mut remaining = values[..lvariant_index]
                .iter()
                .map(|value| value.to_ascii_lowercase())
                .collect::<Vec<_>>();
            if !invalid_private_variants.is_empty() {
                remaining.push("lvariant".to_string());
                remaining.extend(invalid_private_variants.iter().cloned());
            } else if lvariant_index == 0 {
                remaining = Vec::new();
            }
            if !remaining.is_empty() {
                private_use_extension = Some(remaining.join("-"));
            }
        } else {
            private_use_extension = Some(
                values
                    .iter()
                    .map(|value| value.to_ascii_lowercase())
                    .collect::<Vec<_>>()
                    .join("-"),
            );
        }
    }

    // Keep the Java BaseLocale-like identity used by getDisplayName separate
    // from the language/variant values that Locale.toLanguageTag serializes.
    // In particular, no_NO_NY emits nn-NO but still displays as no/NO/NY.
    let display_language = language.clone();
    let display_variants = variants.clone();

    // JVMLocaleLoader.toLocale() discards Locale objects whose language is
    // blank. This includes root, undetermined, and private-use-only tags.
    if language == "und" {
        return None;
    }

    if !unicode_attributes.is_empty() || !unicode_keywords.is_empty() {
        canonical_extensions.push((
            'u',
            unicode_extension_value(&unicode_attributes, &unicode_keywords),
        ));
    }

    // Locale.forLanguageTag preserves the historical Locale(no, NO, NY)
    // compatibility mapping as Norwegian Nynorsk. The legacy variant arrives
    // through the private-use lvariant bridge because NY is not a BCP 47
    // variant subtag.
    let legacy_no_ny = language == "no"
        && region.as_deref() == Some("NO")
        && variants.len() == 1
        && variants[0] == "NY";

    if legacy_no_ny {
        private_use_extension = private_use_extension.and_then(strip_legacy_no_ny_bridge);
    }

    let compatibility_extension = if language == "ja"
        && script.is_none()
        && region.as_deref() == Some("JP")
        && variants == ["JP"]
        && canonical_extensions.is_empty()
        && matches!(private_use_extension.as_deref(), None | Some("lvariant-JP"))
    {
        Some("u-ca-japanese")
    } else if language == "th"
        && script.is_none()
        && region.as_deref() == Some("TH")
        && variants == ["TH"]
        && canonical_extensions.is_empty()
        && matches!(private_use_extension.as_deref(), None | Some("lvariant-TH"))
    {
        Some("u-nu-thai")
    } else {
        None
    };

    let output_language = if legacy_no_ny {
        "nn".to_string()
    } else {
        language.clone()
    };
    let output_variants = if legacy_no_ny {
        Vec::new()
    } else {
        variants.clone()
    };
    let mut canonical_parts = Vec::new();
    if legacy_no_ny {
        canonical_parts.push("nn".to_string());
        if let Some(script) = &script {
            canonical_parts.push(script.clone());
        }
        if let Some(region) = &region {
            canonical_parts.push(region.clone());
        }
    } else {
        if language != "und" {
            canonical_parts.push(language.clone());
        }
        if let Some(script) = &script {
            canonical_parts.push(script.clone());
        }
        if let Some(region) = &region {
            canonical_parts.push(region.clone());
        }
        canonical_parts.extend(
            variants
                .iter()
                .filter(|variant| is_variant_subtag(variant))
                .cloned(),
        );
    }

    if let Some(extension) = compatibility_extension {
        canonical_parts.push(extension.to_string());
        if let Some(private_use) = private_use_extension {
            canonical_parts.push(format!("x-{private_use}"));
        }
    } else {
        canonical_extensions.sort_by_key(|(singleton, _)| *singleton);
        for (singleton, value) in canonical_extensions {
            canonical_parts.push(format!("{singleton}-{value}"));
        }
        if let Some(private_use) = private_use_extension {
            canonical_parts.push(format!("x-{private_use}"));
        }
    }

    let display_base = base_locale_identity(
        &display_language,
        script.as_deref(),
        region.as_deref(),
        &display_variants,
    );
    let canonical_base = base_locale_identity(
        &output_language,
        script.as_deref(),
        region.as_deref(),
        &output_variants,
    );

    Some(ParsedLanguageTag {
        canonical: canonical_parts.join("-"),
        language: output_language,
        display_language,
        display_base,
        canonical_base,
        script,
        region,
        variants: output_variants,
        display_variants,
        unicode_attributes,
        unicode_keywords,
    })
}

fn base_locale_identity(
    language: &str,
    script: Option<&str>,
    region: Option<&str>,
    variants: &[String],
) -> String {
    let mut parts = vec![language.to_string()];
    if let Some(script) = script {
        parts.push(script.to_string());
    }
    if let Some(region) = region {
        parts.push(region.to_string());
    }
    parts.extend(variants.iter().cloned());
    parts.join("-")
}

fn strip_legacy_no_ny_bridge(value: String) -> Option<String> {
    const BRIDGE: &str = "-lvariant-NY";
    if value.eq_ignore_ascii_case("lvariant-NY") {
        return None;
    }
    let prefix_end = value.len().checked_sub(BRIDGE.len())?;
    let (prefix, suffix) = value.split_at(prefix_end);
    if suffix.eq_ignore_ascii_case(BRIDGE) && !prefix.is_empty() {
        Some(prefix.to_string())
    } else {
        Some(value)
    }
}

fn grandfathered_alias(parts: &[&str]) -> Option<&'static str> {
    let tag = parts.join("-").to_ascii_lowercase();
    Some(match tag.as_str() {
        "art-lojban" => "jbo",
        "cel-gaulish" => "xtg-x-cel-gaulish",
        "en-gb-oed" => "en-GB-x-oed",
        "i-ami" => "ami",
        "i-bnn" => "bnn",
        "i-default" => "en-x-i-default",
        "i-enochian" => "und-x-i-enochian",
        "i-hak" => "hak",
        "i-klingon" => "tlh",
        "i-lux" => "lb",
        "i-mingo" => "see-x-i-mingo",
        "i-navajo" => "nv",
        "i-pwn" => "pwn",
        "i-tao" => "tao",
        "i-tay" => "tay",
        "i-tsu" => "tsu",
        "no-bok" => "nb",
        "no-nyn" => "nn",
        "sgn-be-fr" => "sfb",
        "sgn-be-nl" => "vgt",
        "sgn-ch-de" => "sgg",
        "zh-guoyu" => "cmn",
        "zh-hakka" => "hak",
        "zh-min" => "nan-x-zh-min",
        "zh-min-nan" => "nan",
        "zh-xiang" => "hsn",
        _ => return None,
    })
}

fn canonical_language(language: &str) -> String {
    match language.to_ascii_lowercase().as_str() {
        "iw" => "he".to_string(),
        "in" => "id".to_string(),
        "ji" => "yi".to_string(),
        language => language.to_string(),
    }
}

fn is_extension_singleton(value: &str) -> bool {
    value.len() == 1 && is_alpha_subtag(value) && !value.eq_ignore_ascii_case("x")
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

fn parse_unicode_extension(values: &[String]) -> (Vec<String>, Vec<(String, String)>) {
    let mut index = 0;
    let mut attributes = Vec::new();
    while let Some(value) = values.get(index) {
        if value.len() < 3 || value.len() > 8 {
            break;
        }
        attributes.push(value.to_ascii_lowercase());
        index += 1;
    }
    attributes.sort();
    attributes.dedup();

    let mut keywords = Vec::new();
    while let Some(key) = values.get(index) {
        if key.len() != 2 || !is_alphanumeric_subtag(key) {
            break;
        }
        let key = key.to_ascii_lowercase();
        index += 1;
        let start = index;
        while let Some(value) = values.get(index) {
            if value.len() < 3 || value.len() > 8 {
                break;
            }
            index += 1;
        }
        let value = values[start..index].join("-");
        if !keywords.iter().any(|(existing, _)| *existing == key) {
            keywords.push((key, value));
        }
    }
    keywords.sort_by(|left, right| left.0.cmp(&right.0));
    (attributes, keywords)
}

fn unicode_extension_value(attributes: &[String], keywords: &[(String, String)]) -> String {
    let mut values = attributes.to_vec();
    values.extend(keywords.iter().flat_map(|(key, value)| {
        std::iter::once(key.clone()).chain((!value.is_empty()).then(|| value.clone()))
    }));
    values.join("-")
}

fn titlecase_ascii(value: &str) -> String {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    first.to_ascii_uppercase().to_string() + &chars.as_str().to_ascii_lowercase()
}

fn to_fallback_locale(parsed: &ParsedLanguageTag) -> IrDocumentLocale {
    let base = find_tag_record(&parsed.display_language);
    let localized_base = base.map_or(parsed.display_language.as_str(), |record| {
        record.localized_name
    });
    let mut components = Vec::new();
    if let Some(script) = &parsed.script {
        components.push(
            display_data_value(parsed, script)
                .map(str::to_string)
                .or_else(|| {
                    locale_component(&format!("{}-{script}", parsed.display_language), base)
                })
                .unwrap_or_else(|| script.to_string()),
        );
    }
    if let Some(region) = &parsed.region {
        components.push(if region == "001" {
            "World".to_string()
        } else {
            display_data_value(parsed, region)
                .map(str::to_string)
                .or_else(|| {
                    locale_component(&format!("{}-{region}", parsed.display_language), base)
                })
                .or_else(|| locale_component(&format!("en-{region}"), find_tag_record("en")))
                .or_else(|| english_region_name(region))
                .unwrap_or_else(|| region.clone())
        });
    }
    components.extend(
        parsed
            .display_variants
            .iter()
            .map(|variant| display_variant(parsed, variant)),
    );
    components.extend(
        parsed
            .unicode_attributes
            .iter()
            .map(|attribute| display_unicode_attribute(parsed, attribute)),
    );
    components.extend(
        parsed
            .unicode_keywords
            .iter()
            .map(|(key, value)| display_unicode_keyword(parsed, key, value)),
    );
    IrDocumentLocale {
        tag: parsed.canonical.clone(),
        localized_name: format_display_name(parsed, localized_base, &components),
    }
}

fn locale_component(tag: &str, base: Option<&LocaleRecord>) -> Option<String> {
    let record = find_tag_record(tag)?;
    let base = base?;
    let prefix = format!("{} (", base.localized_name);
    record
        .localized_name
        .strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix(')'))
        .map(str::to_string)
}

fn english_region_name(region: &str) -> Option<String> {
    LOCALE_TAG_RECORDS.iter().find_map(|record| {
        let (language, candidate) = record.tag.split_once('-')?;
        if candidate != region {
            return None;
        }
        let base = find_tag_record(language)?;
        let prefix = format!("{} (", base.display_name);
        record
            .display_name
            .strip_prefix(&prefix)
            .and_then(|value| value.strip_suffix(')'))
            .map(str::to_string)
    })
}

fn display_data_value(parsed: &ParsedLanguageTag, key: &str) -> Option<&'static str> {
    let snapshot = DisplaySnapshot::parse(LOCALE_DISPLAY_DATA)?;
    let mut profiles = Vec::with_capacity(LOCALE_DISPLAY_FALLBACK_ORDER.len());
    for profile_kind in LOCALE_DISPLAY_FALLBACK_ORDER {
        let profile = match *profile_kind {
            "language-script-region" => parsed
                .script
                .as_ref()
                .zip(parsed.region.as_ref())
                .map(|(script, region)| format!("{}-{script}-{region}", parsed.language)),
            "language-script" => parsed
                .script
                .as_ref()
                .map(|script| format!("{}-{script}", parsed.language)),
            "language-region" => parsed
                .region
                .as_ref()
                .map(|region| format!("{}-{region}", parsed.language)),
            "language" => Some(parsed.language.clone()),
            "en" => Some(String::from("en")),
            "root" => Some(String::new()),
            _ => None,
        };
        if let Some(profile) = profile.filter(|profile| !profiles.contains(profile)) {
            profiles.push(profile);
        }
    }
    profiles.iter().find_map(|profile| {
        snapshot
            .profile_id(profile)
            .and_then(|profile_id| snapshot.resolve_profile(profile_id, key))
    })
}

fn display_variant(parsed: &ParsedLanguageTag, variant: &str) -> String {
    let key = format!("%%{variant}");
    display_data_value(parsed, &key)
        .unwrap_or(variant)
        .to_string()
}

fn display_unicode_attribute(parsed: &ParsedLanguageTag, attribute: &str) -> String {
    let key = format!("key.{attribute}");
    display_data_value(parsed, &key)
        .unwrap_or(attribute)
        .to_string()
}

fn display_unicode_keyword(parsed: &ParsedLanguageTag, key: &str, value: &str) -> String {
    let key_name = format!("key.{key}");
    let type_name = format!("type.{key}.{value}");
    if let Some(display_type) = display_data_value(parsed, &type_name).filter(|name| *name != value)
    {
        return display_type.to_string();
    }
    // Locale.getDisplayName has three provider fallbacks for Unicode keyword
    // types that are not LocaleNames `type.*` records. The checked-in display
    // dataset contains their JDK-25 provider values; the branches select the
    // data family generically rather than enumerating keyword values.
    let display_type = if key == "cu" {
        let currency_key = format!("currency.{value}");
        display_data_value(parsed, &currency_key).unwrap_or(value)
    } else if key == "rg"
        && value.len() == 6
        && value.as_bytes()[..2].iter().all(u8::is_ascii_alphabetic)
        && value[2..].eq_ignore_ascii_case("zzzz")
    {
        let region = value[..2].to_ascii_uppercase();
        display_data_value(parsed, &region).unwrap_or(value)
    } else if key == "tz" {
        let timezone_key = format!("timezone.{value}");
        display_data_value(parsed, &timezone_key).unwrap_or(value)
    } else {
        value
    };
    let display_key = display_data_value(parsed, &key_name).unwrap_or(key);
    let pattern = display_data_value(parsed, "ListKeyTypePattern").unwrap_or("{0}: {1}");
    format_message_pair(pattern, display_key, display_type)
}

fn format_display_list(parsed: &ParsedLanguageTag, values: &[String]) -> String {
    if values.is_empty() {
        return String::new();
    }
    let pattern = display_data_value(parsed, "ListCompositionPattern").unwrap_or("{0}, {1}");
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

fn format_display_name(parsed: &ParsedLanguageTag, main: &str, qualifiers: &[String]) -> String {
    if qualifiers.is_empty() {
        return main.to_string();
    }
    let list = format_display_list(parsed, qualifiers);
    let pattern = display_data_value(parsed, "DisplayNamePattern");
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
    use super::{find_tag_record, parse_language_tag, resolve, string_equals_ignore_case};
    use super::{
        read_u32, DisplaySnapshot, LOCALE_AVAILABLE_RECORD_COUNT, LOCALE_DATASET_SOURCE_SHA256,
        LOCALE_DATASET_VERSION, LOCALE_DISPLAY_COMPACT_FORMAT_VERSION,
        LOCALE_DISPLAY_COMPACT_RECORD_COUNT, LOCALE_DISPLAY_COMPACT_SHA256,
        LOCALE_DISPLAY_COMPACT_SNAPSHOT_BYTES, LOCALE_DISPLAY_DATA,
        LOCALE_DISPLAY_GENERATED_SOURCE_BYTES, LOCALE_DISPLAY_KEY_COUNT, LOCALE_DISPLAY_MAGIC,
        LOCALE_DISPLAY_MAX_COMPACT_SNAPSHOT_BYTES, LOCALE_DISPLAY_MAX_GENERATED_SOURCE_BYTES,
        LOCALE_DISPLAY_NUMERIC_INDEX_BYTES, LOCALE_DISPLAY_ORACLE_RECORD_COUNT,
        LOCALE_DISPLAY_PROFILE_COUNT, LOCALE_DISPLAY_RAW_STRING_POOL_BYTES,
        LOCALE_DISPLAY_RECORD_COUNT, LOCALE_DISPLAY_SOURCE_SHA256, LOCALE_DISPLAY_VALUE_COUNT,
        LOCALE_NAME_RECORDS, LOCALE_TAG_RECORDS, LOCALE_TAG_RECORD_COUNT,
    };

    #[test]
    fn dataset_identity_and_indexes_are_guarded() {
        assert_eq!(LOCALE_AVAILABLE_RECORD_COUNT, LOCALE_NAME_RECORDS.len());
        assert_eq!(LOCALE_TAG_RECORD_COUNT, LOCALE_TAG_RECORDS.len());
        assert_eq!(LOCALE_TAG_RECORDS.len(), 1015);
        assert_eq!(LOCALE_NAME_RECORDS.len(), 1016);
        assert_eq!(LOCALE_NAME_RECORDS[0].tag, "he");
        assert_eq!(LOCALE_TAG_RECORDS[0].tag, "af");
        assert_eq!(LOCALE_TAG_RECORDS.last().unwrap().tag, "zu-ZA");
        assert_eq!(LOCALE_DATASET_VERSION, "17.0.20.1+1");
        assert_eq!(
            LOCALE_DATASET_SOURCE_SHA256,
            "a21268dd1fb3cc6fd5cea32b52fa63099eb390a7e82c27636195db1086d645fd"
        );
        assert_eq!(LOCALE_DISPLAY_RECORD_COUNT, 308533);
        assert_eq!(
            LOCALE_DISPLAY_RECORD_COUNT,
            LOCALE_DISPLAY_ORACLE_RECORD_COUNT
        );
        assert_eq!(
            LOCALE_DISPLAY_SOURCE_SHA256,
            "03d633326dc30ac8423cfb14b4bc0d3fa4f35e7a86575e8eefbdf540c620d489"
        );
        assert_eq!(LOCALE_DISPLAY_COMPACT_RECORD_COUNT, 152731);
        assert_eq!(LOCALE_DISPLAY_PROFILE_COUNT, 287);
        assert_eq!(LOCALE_DISPLAY_KEY_COUNT, 1569);
        assert_eq!(LOCALE_DISPLAY_VALUE_COUNT, 88024);
        assert_eq!(LOCALE_DISPLAY_RAW_STRING_POOL_BYTES, 2045327);
        assert_eq!(LOCALE_DISPLAY_NUMERIC_INDEX_BYTES, 1226720);
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
        assert_eq!(
            LOCALE_NAME_RECORDS
                .iter()
                .filter(|record| record.tag == "nn-NO")
                .count(),
            2,
            "the only available-order tag collision is explicitly modeled"
        );
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
    fn every_reference_tag_resolves_to_the_canonical_snapshot() {
        for record in LOCALE_TAG_RECORDS {
            let resolved = resolve(record.tag).expect("reference tag should resolve");
            assert_eq!(resolved.tag, record.tag);
            assert_eq!(resolved.localized_name, record.localized_name);
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
    fn name_matching_reuses_unicode_13_characterwise_case() {
        assert!(string_equals_ignore_case("English", "eNgLiSh"));
        assert!(string_equals_ignore_case("İ", "i"));
        assert!(!string_equals_ignore_case("É", "e\u{301}"));
    }

    #[test]
    fn canonicalizes_aliases_and_structured_tags() {
        assert_eq!(parse_language_tag("EN-us").unwrap().canonical, "en-US");
        assert_eq!(parse_language_tag("iw").unwrap().canonical, "he");
        for (input, expected) in [
            ("ar-aao", "aao"),
            ("es-abc", "abc"),
            ("de-foo", "foo"),
            ("abcd-efg", "efg"),
        ] {
            assert_eq!(parse_language_tag(input).unwrap().canonical, expected);
        }
        assert_eq!(
            parse_language_tag("ar-aao-Latn-EG").unwrap().canonical,
            "aao-Latn-EG"
        );
        assert_eq!(parse_language_tag("i-klingon").unwrap().canonical, "tlh");
        assert_eq!(parse_language_tag("zh-cmn").unwrap().canonical, "cmn");
        assert_eq!(parse_language_tag("zh-yue").unwrap().canonical, "yue");
        assert_eq!(parse_language_tag("zh-guoyu").unwrap().canonical, "cmn");
        assert_eq!(parse_language_tag("zh-hakka").unwrap().canonical, "hak");
        assert_eq!(
            parse_language_tag("zh-min").unwrap().canonical,
            "nan-x-zh-min"
        );
        assert_eq!(
            parse_language_tag("no-NO-x-lvariant-NY").unwrap().canonical,
            "nn-NO"
        );
        assert_eq!(
            parse_language_tag("NO-no-x-LVARIANT-ny").unwrap().canonical,
            "no-NO-x-lvariant-ny"
        );
        assert_eq!(
            parse_language_tag("no-Latn-NO-x-lvariant-NY")
                .unwrap()
                .canonical,
            "nn-Latn-NO"
        );
        assert_eq!(
            parse_language_tag("no-NO-u-ca-gregory-x-lvariant-NY")
                .unwrap()
                .canonical,
            "nn-NO-u-ca-gregory"
        );
        assert_eq!(
            parse_language_tag("no-NO-x-foo-lvariant-NY")
                .unwrap()
                .canonical,
            "nn-NO-x-foo"
        );
        assert_eq!(
            parse_language_tag("no-NO-u-ca-gregory-x-foo-lvariant-NY")
                .unwrap()
                .canonical,
            "nn-NO-u-ca-gregory-x-foo"
        );
        assert_eq!(parse_language_tag("no-NO-NY").unwrap().canonical, "no-NO");
        assert_eq!(parse_language_tag("zh-min-nan").unwrap().canonical, "nan");
        assert_eq!(parse_language_tag("zh-xiang").unwrap().canonical, "hsn");
        assert_eq!(
            parse_language_tag("i-mingo").unwrap().canonical,
            "see-x-i-mingo"
        );
        assert_eq!(
            parse_language_tag("zh-hant-tw").unwrap().canonical,
            "zh-Hant-TW"
        );
        assert_eq!(
            parse_language_tag("de-DE-1996").unwrap().canonical,
            "de-DE-1996"
        );
    }

    #[test]
    fn legacy_no_ny_uses_the_derived_base_locale_for_localized_names() {
        for (input, expected_tag, expected_name) in [
            ("no-NO-x-lvariant-NY", "nn-NO", "norsk (Noreg, nynorsk)"),
            (
                "no-NO-x-foo-lvariant-NY",
                "nn-NO-x-foo",
                "norsk (Noreg, nynorsk)",
            ),
            (
                "no-NO-u-ca-gregory-x-lvariant-NY",
                "nn-NO-u-ca-gregory",
                "norsk (Noreg, nynorsk, kalender: gregory)",
            ),
            (
                "no-NO-u-ca-gregory-x-foo-lvariant-NY",
                "nn-NO-u-ca-gregory-x-foo",
                "norsk (Noreg, nynorsk, kalender: gregory)",
            ),
            (
                "no-Latn-NO-x-lvariant-NY",
                "nn-Latn-NO",
                "norsk (latinsk, Noreg, nynorsk)",
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
    fn canonicalizes_private_use_variants_and_extension_order_like_java() {
        assert_eq!(
            parse_language_tag("en-US-x-lvariant-POSIX")
                .unwrap()
                .canonical,
            "en-US-POSIX"
        );
        assert_eq!(
            parse_language_tag("en-x-abc-lvariant-Abcde-Defgh")
                .unwrap()
                .canonical,
            "en-Abcde-Defgh-x-abc"
        );
        assert_eq!(
            parse_language_tag("en-x-lvariant-Abc").unwrap().canonical,
            "en-x-lvariant-Abc"
        );
        assert_eq!(
            parse_language_tag("de-POSIX-x-URP-lvariant-Abcde-Defgh")
                .unwrap()
                .canonical,
            "de-POSIX-Abcde-Defgh-x-urp"
        );
        assert_eq!(
            parse_language_tag("en-b-foo-a-bar").unwrap().canonical,
            "en-a-bar-b-foo"
        );
        assert_eq!(
            parse_language_tag("en-u-nu-latn-ca-gregory")
                .unwrap()
                .canonical,
            "en-u-ca-gregory-nu-latn"
        );
        assert_eq!(
            parse_language_tag("en-u-ca-gregory-ca-buddhist")
                .unwrap()
                .canonical,
            "en-u-ca-gregory"
        );
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
    fn rejects_root_only_and_malformed_tags() {
        for identifier in ["", "   ", "en_US", "x-private", "und"] {
            assert!(resolve(identifier).is_none(), "identifier: {identifier}");
        }
        assert_eq!(resolve("en--US").unwrap().tag, "en");
        assert_eq!(resolve("en-").unwrap().tag, "en");
        assert_eq!(
            resolve("ja-JP-x-lvariant-JP").unwrap().tag,
            "ja-JP-u-ca-japanese-x-lvariant-JP"
        );
        assert_eq!(resolve("no-NO-x-lvariant-NY").unwrap().tag, "nn-NO");
        assert!(find_tag_record("not-a-real-tag").is_none());
    }
}
