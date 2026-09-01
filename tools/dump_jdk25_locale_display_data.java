import java.util.Currency;
import java.util.Locale;
import java.util.Map;
import java.util.Optional;
import java.util.TreeMap;
import java.util.TimeZone;
import sun.util.resources.LocaleData;
import sun.util.resources.OpenListResourceBundle;
import sun.util.locale.provider.LocaleProviderAdapter;
import sun.util.locale.provider.TimeZoneNameUtility;

/** Emits deterministic effective locale-name display data for the .doclang generator. */
final class DumpJdk25LocaleDisplayData {
    private DumpJdk25LocaleDisplayData() {}

    public static void main(String[] args) {
        // JDK 25 uses CLDR as the locale-data provider and retains the small
        // FALLBACK adapter for root/default provider values. LocaleNameProvider
        // lookup tries CLDR and then FALLBACK; both layers are therefore part
        // of the effective oracle.
        LocaleData cldr = new LocaleData(LocaleProviderAdapter.Type.CLDR);
        LocaleData fallback = new LocaleData(LocaleProviderAdapter.Type.FALLBACK);
        TreeMap<String, String> rows = new TreeMap<>();
        TreeMap<String, Locale> profiles = new TreeMap<>();
        TreeMap<String, String> timezoneIds = new TreeMap<>();
        boolean readingTimezoneIds = false;
        for (String arg : args) {
            if (arg.equals("--timezone")) {
                readingTimezoneIds = true;
            } else if (readingTimezoneIds) {
                timezoneIds.put(arg, arg);
            } else {
                throw new IllegalArgumentException("unknown argument: " + arg);
            }
        }

        addBundle(Locale.ROOT, cldr, rows, profiles, false);
        for (Locale locale : Locale.getAvailableLocales()) {
            addBundle(locale, cldr, rows, profiles, false);
        }
        // FALLBACK supports the root locale only. Add only keys absent from
        // CLDR so the effective provider precedence remains CLDR first.
        addBundle(Locale.ROOT, fallback, rows, profiles, true);
        addCurrencyData(profiles, rows);
        addTimezoneData(profiles, timezoneIds, rows);

        for (Map.Entry<String, String> row : rows.entrySet()) {
            String[] key = row.getKey().split("\\t", 2);
            System.out.println("display\t" + key[0] + "\t" + key[1] + "\t" + row.getValue());
        }
    }

    private static void addBundle(
            Locale requestedLocale,
            LocaleData data,
            TreeMap<String, String> rows,
            TreeMap<String, Locale> profiles,
            boolean preserveExisting
    ) {
        OpenListResourceBundle bundle = data.getLocaleNames(requestedLocale);
        // ResourceBundle may label the FALLBACK root bundle as "und", but
        // provider lookup addresses it as Locale.ROOT. Keep one canonical
        // root profile so CLDR-first/FALLBACK-second composition matches the
        // public LocaleNameProvider path.
        String profile = requestedLocale.equals(Locale.ROOT)
                ? ""
                : bundle.getLocale().toLanguageTag();
        if (preserveExisting) {
            profiles.putIfAbsent(profile, bundle.getLocale());
        } else {
            profiles.put(profile, bundle.getLocale());
        }
        for (String key : bundle.keySet()) {
            if (!isDisplayDataKey(key)) {
                continue;
            }
            Object value = bundle.getObject(key);
            if (!(value instanceof String stringValue)) {
                throw new IllegalStateException("locale display data is not a string: " + key);
            }
            rejectControlCharacters(profile, "profile");
            rejectControlCharacters(key, "key");
            rejectControlCharacters(stringValue, "value");
            String rowKey = profile + "\t" + key;
            String previous = rows.putIfAbsent(rowKey, stringValue);
            if (!preserveExisting && previous != null && !previous.equals(stringValue)) {
                throw new IllegalStateException("conflicting locale display data: " + rowKey);
            }
        }
    }

    private static void addCurrencyData(
            TreeMap<String, Locale> profiles,
            TreeMap<String, String> rows
    ) {
        TreeMap<String, Currency> currencies = new TreeMap<>();
        for (Currency currency : Currency.getAvailableCurrencies()) {
            currencies.put(currency.getCurrencyCode(), currency);
        }
        for (Map.Entry<String, Locale> profile : profiles.entrySet()) {
            for (Map.Entry<String, Currency> currency : currencies.entrySet()) {
                String value = currency.getValue().getDisplayName(profile.getValue());
                putRow(rows, profile.getKey(), "currency." + currency.getKey().toLowerCase(Locale.ROOT), value);
            }
        }
    }

    private static void addTimezoneData(
            TreeMap<String, Locale> profiles,
            TreeMap<String, String> timezoneIds,
            TreeMap<String, String> rows
    ) {
        for (Map.Entry<String, Locale> profile : profiles.entrySet()) {
            for (String shortId : timezoneIds.keySet()) {
                Optional<String> canonicalId = TimeZoneNameUtility.convertLDMLShortID(shortId);
                if (canonicalId.isEmpty()) {
                    continue;
                }
                String displayName = TimeZoneNameUtility.retrieveGenericDisplayName(
                        canonicalId.get(), TimeZone.LONG, profile.getValue());
                if (displayName != null) {
                    putRow(rows, profile.getKey(), "timezone." + shortId, displayName);
                }
            }
        }
    }

    private static void putRow(
            TreeMap<String, String> rows,
            String profile,
            String key,
            String value
    ) {
        rejectControlCharacters(profile, "profile");
        rejectControlCharacters(key, "key");
        rejectControlCharacters(value, "value");
        String rowKey = profile + "\t" + key;
        String previous = rows.putIfAbsent(rowKey, value);
        if (previous != null && !previous.equals(value)) {
            throw new IllegalStateException("conflicting locale display data: " + rowKey);
        }
    }

    private static boolean isDisplayDataKey(String key) {
        return key.equals("DisplayNamePattern")
            || key.equals("ListCompositionPattern")
            || key.equals("ListKeyTypePattern")
            || key.startsWith("key.")
            || key.startsWith("type.")
            || key.startsWith("%%")
            || key.matches("[a-z]{2,3}")
            || key.matches("[A-Za-z]{4}")
            || key.matches("[A-Z]{2}|[0-9]{3}");
    }

    private static void rejectControlCharacters(String value, String label) {
        if (value.indexOf('\t') >= 0 || value.indexOf('\n') >= 0 || value.indexOf('\r') >= 0) {
            throw new IllegalStateException("locale display " + label + " contains a TSV control character");
        }
    }
}
