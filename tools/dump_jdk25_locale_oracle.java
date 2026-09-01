import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Locale;
import java.util.ResourceBundle;
import java.util.TreeSet;

/** Emits public JVM behavior for the bounded Quarkdown v2.5.1 .doclang surface. */
final class DumpJdk25LocaleOracle {
    private DumpJdk25LocaleOracle() {}

    public static void main(String[] args) {
        List<Locale> availableLocales = orderedAvailableLocales();
        TreeSet<String> requests = new TreeSet<>();
        for (Locale locale : availableLocales) {
            if (!locale.getLanguage().isBlank()) {
                requests.add(locale.toLanguageTag());
                requests.add(locale.getDisplayName(Locale.ENGLISH));
                String mixedCaseName = mixedCase(locale.getDisplayName(Locale.ENGLISH));
                if (!mixedCaseName.equals(locale.getDisplayName(Locale.ENGLISH))) {
                    requests.add(mixedCaseName);
                }
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
                "sr-Latn-RS-1994-x-private",
                "no-NO-x-foo-lvariant-NY",
                "no-NO-u-ca-gregory-x-lvariant-NY",
                "no-NO-u-ca-gregory-x-foo-lvariant-NY",
                "no-NO-x-lvariant-ny",
                "no-NO-NY",
                "en--US",
                "en-u",
                "en-u-ca",
                "x-private",
                "und"
        ));
        addDeterministicStructuredRequests(requests);

        ResourceBundle.Control control = ResourceBundle.Control.getControl(ResourceBundle.Control.FORMAT_DEFAULT);
        for (String request : requests) {
            Locale locale = findByEnglishName(request, availableLocales);
            boolean nameMatch = locale != null;
            if (locale == null) {
                locale = Locale.forLanguageTag(request);
            }
            if (locale.getLanguage().isBlank()) {
                reject(request);
                System.out.println("locale\t" + request + "\tunresolved\t\t\t");
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

    private static Locale findByEnglishName(String name, List<Locale> availableLocales) {
        for (Locale locale : availableLocales) {
            if (locale.getDisplayName(Locale.ENGLISH).equalsIgnoreCase(name)) {
                return locale;
            }
        }
        return null;
    }

    private static List<Locale> orderedAvailableLocales() {
        // The JDK API does not promise an order for this provider union. Pin
        // the exact returned set to a stable order before modeling Quarkdown's
        // first English-name match.
        ArrayList<Locale> locales = new ArrayList<>(List.of(Locale.getAvailableLocales()));
        locales.sort(Comparator
                .comparing(Locale::toLanguageTag)
                .thenComparing(Locale::getLanguage)
                .thenComparing(Locale::getScript)
                .thenComparing(Locale::getCountry)
                .thenComparing(Locale::getVariant)
                .thenComparing(Locale::toString));
        return locales;
    }

    private static String mixedCase(String value) {
        StringBuilder result = new StringBuilder(value.length());
        boolean upper = false;
        for (int index = 0; index < value.length(); index++) {
            char character = value.charAt(index);
            if (character >= 'a' && character <= 'z') {
                result.append(upper ? Character.toUpperCase(character) : character);
                upper = !upper;
            } else if (character >= 'A' && character <= 'Z') {
                result.append(upper ? character : Character.toLowerCase(character));
                upper = !upper;
            } else {
                result.append(character);
            }
        }
        return result.toString();
    }

    private static void addDeterministicStructuredRequests(TreeSet<String> requests) {
        String[] languages = {"en", "fr", "de", "zh", "no", "nb", "nn", "sr", "ar", "ja", "th", "xx"};
        String[] scripts = {"", "Latn", "Hans", "Hant", "Cyrl"};
        String[] regions = {"", "US", "CA", "CN", "TW", "HK", "NO", "RS", "001"};
        String[] variants = {"", "POSIX", "1996", "rozaj", "biske"};
        String[] extensions = {
            "", "-u-ca-gregory", "-u-ca-buddhist", "-u-nu-arab",
            "-u-ca-gregory-nu-latn", "-a-foo", "-a-foo-b-bar",
            "-x-private", "-x-foo-lvariant-NY", "-u-ca-gregory-x-foo-lvariant-NY"
        };
        long state = 0x6a09e667f3bcc909L;
        for (int index = 0; index < 4096; index++) {
            state = state * 6364136223846793005L + 1442695040888963407L;
            int language = (int) ((state >>> 32) % languages.length);
            state = state * 6364136223846793005L + 1442695040888963407L;
            int script = (int) ((state >>> 32) % scripts.length);
            state = state * 6364136223846793005L + 1442695040888963407L;
            int region = (int) ((state >>> 32) % regions.length);
            state = state * 6364136223846793005L + 1442695040888963407L;
            int variant = (int) ((state >>> 32) % variants.length);
            state = state * 6364136223846793005L + 1442695040888963407L;
            int extension = (int) ((state >>> 32) % extensions.length);
            StringBuilder request = new StringBuilder(languages[language]);
            if (!scripts[script].isEmpty()) {
                request.append('-').append(scripts[script]);
            }
            if (!regions[region].isEmpty()) {
                request.append('-').append(regions[region]);
            }
            if (!variants[variant].isEmpty()) {
                request.append('-').append(variants[variant]);
            }
            request.append(extensions[extension]);
            requests.add(request.toString());
        }
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
