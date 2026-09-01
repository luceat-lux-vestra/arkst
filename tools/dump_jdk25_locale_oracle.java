import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.ResourceBundle;
import java.util.TreeSet;

/** Emits public JVM behavior for the bounded Quarkdown v2.5.1 .doclang surface. */
final class DumpJdk25LocaleOracle {
    private DumpJdk25LocaleOracle() {}

    public static void main(String[] args) {
        TreeSet<String> requests = new TreeSet<>();
        for (Locale locale : Locale.getAvailableLocales()) {
            if (!locale.getLanguage().isBlank()) {
                requests.add(locale.toLanguageTag());
            }
        }
        requests.addAll(List.of(
                "English",
                "eNgLiSh",
                "zh-TW-u-ca-buddhist",
                "zh-CN-u-ca-buddhist",
                "zh-SG-u-ca-buddhist",
                "zh-HK-u-ca-buddhist",
                "zh-MO-u-ca-buddhist",
                "zh-Hans",
                "zh-Hant",
                "zh-Hans-CN",
                "zh-Hant-TW",
                "no",
                "nb",
                "nn",
                "no-NO",
                "nb-NO",
                "nn-NO",
                "no-NO-x-lvariant-NY",
                "no-Latn-NO-x-lvariant-NY",
                "en-Latn-US-POSIX",
                "sl-rozaj-biske-1994",
                "de-DE-1901-u-ca-gregory",
                "sr-Latn-RS-1994-x-private"
        ));

        ResourceBundle.Control control = ResourceBundle.Control.getControl(ResourceBundle.Control.FORMAT_DEFAULT);
        for (String request : requests) {
            Locale locale = findByEnglishName(request);
            boolean nameMatch = locale != null;
            if (locale == null) {
                locale = Locale.forLanguageTag(request);
            }
            if (locale.getLanguage().isBlank()) {
                continue;
            }
            List<String> candidates = new ArrayList<>();
            if (!nameMatch) {
                for (Locale candidate : control.getCandidateLocales("scribium", locale)) {
                    candidates.add(candidate == Locale.ROOT ? "<root>" : baseIdentity(candidate));
                }
            }
            reject(request);
            reject(locale.toLanguageTag());
            String localizedName = locale.getDisplayName(locale);
            reject(localizedName);
            for (String candidate : candidates) {
                reject(candidate);
            }
            System.out.println("locale\t" + request + "\t" + (nameMatch ? "name" : "tag") + "\t"
                    + locale.toLanguageTag() + "\t" + localizedName + "\t" + String.join("|", candidates));
        }
    }

    private static Locale findByEnglishName(String name) {
        for (Locale locale : Locale.getAvailableLocales()) {
            if (locale.getDisplayName(Locale.ENGLISH).equalsIgnoreCase(name)) {
                return locale;
            }
        }
        return null;
    }

    /** Stable text form of the ResourceBundle candidate's BaseLocale fields. */
    private static String baseIdentity(Locale locale) {
        List<String> parts = new ArrayList<>();
        if (!locale.getLanguage().isBlank()) {
            parts.add(locale.getLanguage());
        }
        if (!locale.getScript().isBlank()) {
            parts.add(locale.getScript());
        }
        if (!locale.getCountry().isBlank()) {
            parts.add(locale.getCountry());
        }
        if (!locale.getVariant().isBlank()) {
            String[] variants = locale.getVariant().split("_", -1);
            int valid = 0;
            while (valid < variants.length && isVariant(variants[valid])) {
                parts.add(variants[valid]);
                valid++;
            }
            if (valid < variants.length) {
                parts.add("x");
                parts.add("lvariant");
                for (int index = valid; index < variants.length; index++) {
                    parts.add(variants[index]);
                }
            }
        }
        return String.join("-", parts);
    }

    private static boolean isVariant(String value) {
        if (value.length() >= 5 && value.length() <= 8) {
            return value.chars().allMatch(Character::isLetterOrDigit);
        }
        return value.length() == 4
                && Character.isDigit(value.charAt(0))
                && value.chars().allMatch(Character::isLetterOrDigit);
    }

    private static void reject(String value) {
        if (value.indexOf('\t') >= 0 || value.indexOf('\n') >= 0 || value.indexOf('\r') >= 0
                || value.indexOf('|') >= 0) {
            throw new IllegalStateException("oracle field contains a delimiter");
        }
    }
}
