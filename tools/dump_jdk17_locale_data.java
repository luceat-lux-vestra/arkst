import java.util.Locale;
import java.util.TreeSet;

/** Emits the locale facts consumed by generate_jdk17_locale_data.py. */
final class DumpJdk17LocaleData {
    private DumpJdk17LocaleData() {}

    public static void main(String[] args) {
        System.out.println("runtime.version\t" + Runtime.version());
        System.out.println("java.vendor\t" + System.getProperty("java.vendor"));
        System.out.println("java.vendor.version\t" + System.getProperty("java.vendor.version"));
        System.out.println("java.locale.providers\t" + System.getProperty("java.locale.providers"));

        TreeSet<String> canonicalTags = new TreeSet<>();
        for (Locale locale : Locale.getAvailableLocales()) {
            if (locale.getLanguage().isBlank()) {
                continue;
            }

            String tag = locale.toLanguageTag();
            String displayName = locale.getDisplayName(Locale.ENGLISH);
            String localizedName = locale.getDisplayName(locale);
            rejectControlCharacters(tag, "tag");
            rejectControlCharacters(displayName, "display name");
            rejectControlCharacters(localizedName, "localized name");
            canonicalTags.add(Locale.forLanguageTag(tag).toLanguageTag());
            System.out.println("available\t" + tag + "\t" + displayName + "\t" + localizedName);
        }

        for (String tag : canonicalTags) {
            Locale locale = Locale.forLanguageTag(tag);
            System.out.println(
                "tag\t" + tag + "\t"
                    + locale.getDisplayName(Locale.ENGLISH) + "\t"
                    + locale.getDisplayName(locale)
            );
        }
    }

    private static void rejectControlCharacters(String value, String label) {
        if (value.indexOf('\t') >= 0 || value.indexOf('\n') >= 0 || value.indexOf('\r') >= 0) {
            throw new IllegalStateException("locale " + label + " contains a TSV control character");
        }
    }
}
