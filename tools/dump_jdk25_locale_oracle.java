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
                    candidates.add(candidate == Locale.ROOT ? "<root>" : candidate.toLanguageTag());
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

    private static void reject(String value) {
        if (value.indexOf('\t') >= 0 || value.indexOf('\n') >= 0 || value.indexOf('\r') >= 0
                || value.indexOf('|') >= 0) {
            throw new IllegalStateException("oracle field contains a delimiter");
        }
    }
}
