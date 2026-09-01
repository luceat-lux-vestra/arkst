import java.util.ArrayList;
import java.util.Locale;
import java.util.List;
import java.util.TreeSet;

/** Emits the locale facts consumed by generate_jdk25_locale_data.py. */
final class DumpJdk25LocaleData {
    private DumpJdk25LocaleData() {}

    public static void main(String[] args) {
        System.out.println("runtime.version\t" + Runtime.version());
        System.out.println("java.vendor\t" + System.getProperty("java.vendor"));
        System.out.println("java.vendor.version\t" + System.getProperty("java.vendor.version"));
        System.out.println("java.locale.providers\t" + System.getProperty("java.locale.providers"));

        Locale[] availableLocales = Locale.getAvailableLocales();
        List<CollisionGroup> collisions = findNameCollisions(availableLocales);
        System.out.println("name-collision-count\t" + collisions.size());

        TreeSet<String> canonicalTags = new TreeSet<>();
        for (Locale locale : availableLocales) {
            String tag = locale.toLanguageTag();
            String displayName = locale.getDisplayName(Locale.ENGLISH);
            String localizedName = locale.getDisplayName(locale);
            rejectControlCharacters(tag, "tag");
            rejectControlCharacters(displayName, "display name");
            rejectControlCharacters(localizedName, "localized name");
            canonicalTags.add(Locale.forLanguageTag(tag).toLanguageTag());
            System.out.println("available\t" + tag + "\t" + displayName + "\t" + localizedName
                    + "\t" + locale.getLanguage() + "\t" + locale.getScript() + "\t"
                    + locale.getCountry() + "\t" + locale.getVariant() + "\t"
                    + (locale.getCountry().isEmpty() ? "" : locale.getDisplayCountry(locale)));
        }

        for (CollisionGroup collision : collisions) {
            StringBuilder line = new StringBuilder("collision\t").append(collision.displayName);
            for (Locale locale : collision.locales) {
                line.append('\t').append(locale.toLanguageTag());
            }
            System.out.println(line);
        }

        for (String tag : canonicalTags) {
            Locale locale = Locale.forLanguageTag(tag);
            System.out.println(
                "tag\t" + tag + "\t"
                    + locale.getDisplayName(Locale.ENGLISH) + "\t"
                    + locale.getDisplayName(locale) + "\t"
                    + locale.getLanguage() + "\t" + locale.getScript() + "\t"
                    + locale.getCountry() + "\t" + locale.getVariant() + "\t"
                    + (locale.getCountry().isEmpty() ? "" : locale.getDisplayCountry(locale))
            );
        }
    }

    private static List<CollisionGroup> findNameCollisions(Locale[] availableLocales) {
        List<CollisionGroup> groups = new ArrayList<>();
        for (Locale locale : availableLocales) {
            String displayName = locale.getDisplayName(Locale.ENGLISH);
            CollisionGroup group = null;
            for (CollisionGroup candidate : groups) {
                if (candidate.displayName.equalsIgnoreCase(displayName)) {
                    group = candidate;
                    break;
                }
            }
            if (group == null) {
                group = new CollisionGroup(displayName);
                groups.add(group);
            }
            group.locales.add(locale);
        }
        groups.removeIf(group -> group.locales.size() < 2);
        return groups;
    }

    private static final class CollisionGroup {
        private final String displayName;
        private final List<Locale> locales = new ArrayList<>();

        private CollisionGroup(String displayName) {
            this.displayName = displayName;
        }
    }

    private static void rejectControlCharacters(String value, String label) {
        if (value.indexOf('\t') >= 0 || value.indexOf('\n') >= 0 || value.indexOf('\r') >= 0) {
            throw new IllegalStateException("locale " + label + " contains a TSV control character");
        }
    }
}
