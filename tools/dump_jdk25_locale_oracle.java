import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.ResourceBundle;
import java.util.TreeSet;

/** Emits public JVM behavior for the bounded Quarkdown v2.5.1 .doclang surface. */
final class DumpJdk25LocaleOracle {
    private DumpJdk25LocaleOracle() {}

    public static void main(String[] args) {
        // Quarkdown's name-first path must see the exact array returned by the
        // pinned JVM. Keep this order; only the request set below is sorted for
        // deterministic oracle output.
        List<Locale> availableLocales = List.of(Locale.getAvailableLocales());
        TreeSet<String> requests = new TreeSet<>();
        for (Locale locale : availableLocales) {
            requests.add(locale.toLanguageTag());
            requests.add(locale.getDisplayName(Locale.ENGLISH));
            String mixedCaseName = mixedCase(locale.getDisplayName(Locale.ENGLISH));
            if (!mixedCaseName.equals(locale.getDisplayName(Locale.ENGLISH))) {
                requests.add(mixedCaseName);
            }
        }
        requests.addAll(List.of(
                "English",
                "eNgLiSh",
                "_",
                "-",
                "--",
                "123",
                "@",
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
                "ja-JP-x-lvariant-JP",
                "th-TH-x-lvariant-TH",
                "",
                "en_US",
                "   ",
                "en--US",
                "en-u",
                "en-u-ca",
                "en-a-foo-u",
                "en-a-foo",
                "en-b-foo-a-bar",
                "en-a-foo-a-bar",
                "en-A-foo-a-bar",
                "en-u-ca-gregory",
                "en-u-nu-latn-ca-gregory",
                "en-u-ca-gregory-ca-buddhist",
                "en-u-abc-def",
                "en-u-abc-ca-gregory",
                "ar-aao",
                "ar-aao-Latn-EG",
                "zh-cmn",
                "zh-yue",
                "en-abc",
                "en-abc-def",
                "en-abc-def-ghi",
                "en-POSIX",
                "de-1901",
                "sl-rozaj",
                "sl-rozaj-biske-1994",
                "art-lojban",
                "cel-gaulish",
                "en-GB-oed",
                "i-ami",
                "i-bnn",
                "i-default",
                "i-enochian",
                "i-hak",
                "i-klingon",
                "i-Klingon",
                "i-lux",
                "i-mingo",
                "i-navajo",
                "i-pwn",
                "i-tao",
                "i-tay",
                "i-tsu",
                "no-bok",
                "no-nyn",
                "sgn-BE-FR",
                "ſgn-BE-FR",
                "sgn-BE-NL",
                "sgn-CH-DE",
                "zh-guoyu",
                "zh-hakka",
                "zh-min",
                "zh-min-nan",
                "zh-xiang",
                "und-Latn",
                "und-US",
                "und-001",
                "und-Latn-US",
                "und-x-private",
                "und-u-ca-gregory",
                "x",
                "x-",
                "x-a",
                "x-1",
                "x-ab",
                "x-US",
                "x-abc",
                "x-ABC",
                "x-Latn",
                "x-latn",
                "x-123",
                "x-001",
                "x-abc-def",
                "x-Latn-US",
                "x-US-abc",
                "x-lvariant-NY",
                "x-foo-lvariant-NY",
                "en-x-lvariant-POSIX",
                "en-x-abc-lvariant-Abcde-Defgh",
                "en-x-lvariant-Abc",
                "no-NO-x-lvariant-NY",
                "no-Latn-NO-x-lvariant-NY",
                "no-NO-x-foo-lvariant-NY",
                "no-NO-u-ca-gregory-x-lvariant-NY",
                "no-NO-u-ca-gregory-x-foo-lvariant-NY",
                "no-NO-x-lvariant-ny",
                "ja-JP-x-lvariant-JP",
                "th-TH-x-lvariant-TH",
                "x-private",
                "x-y-z-blork",
                "und"
        ));
        addPrivateUseMatrix(requests);
        addDeterministicStructuredRequests(requests);

        ResourceBundle.Control control = ResourceBundle.Control.getControl(ResourceBundle.Control.FORMAT_DEFAULT);
        for (String request : requests) {
            Locale locale = findByEnglishName(request, availableLocales);
            boolean nameMatch = locale != null;
            if (locale == null) {
                locale = Locale.forLanguageTag(request);
            }
            List<String> candidates = new ArrayList<>();
            if (!nameMatch) {
                for (Locale candidate : control.getCandidateLocales("arkst", locale)) {
                    candidates.add(candidate == Locale.ROOT ? "<root>" : baseIdentity(candidate));
                }
            }
            reject(request);
            reject(locale.toLanguageTag());
            reject(locale.getLanguage());
            String localizedName = locale.getDisplayName(locale);
            reject(localizedName);
            for (String candidate : candidates) {
                reject(candidate);
            }
            String countryCode = locale.getCountry();
            String localizedCountryName = countryCode.isEmpty()
                    ? ""
                    : locale.getDisplayCountry(locale);
            System.out.println("locale\t" + request + "\t" + (nameMatch ? "name" : "tag") + "\t"
                    + locale.toLanguageTag() + "\t" + locale.getLanguage() + "\t"
                    + countryCode + "\t" + locale.getDisplayName(Locale.ENGLISH) + "\t"
                    + localizedName + "\t" + localizedCountryName + "\t"
                    + locale.getLanguage() + "\t" + String.join("|", candidates));
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

    private static void addPrivateUseMatrix(TreeSet<String> requests) {
        String[] subtags = {
            "a", "1", "ab", "US", "abc", "ABC", "123", "001", "Latn", "1234",
            "abcde", "ABCDEF", "a1b2c3d4", "abcdefgh", "foo-bar", "Latn-US", "US-abc"
        };
        for (String subtag : subtags) {
            requests.add("x-" + subtag);
        }
        for (String first : new String[] {"a", "1", "ab", "US", "abc", "Latn", "1234", "abcde"}) {
            for (String second : new String[] {"a", "1", "US", "abc", "Latn", "1234", "abcde"}) {
                requests.add("x-" + first + "-" + second);
            }
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
