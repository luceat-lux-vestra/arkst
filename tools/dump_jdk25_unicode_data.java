/*
 * Generation-only oracle helper for the pinned Eclipse Temurin 25 runtime.
 *
 * This is independently authored Arkst tooling. It calls the public JDK
 * Character/String APIs; it is not linked into the runtime.
 */

import java.io.BufferedReader;
import java.io.IOException;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.Locale;

final class DumpJdk25UnicodeData {
    private DumpJdk25UnicodeData() {}

    public static void main(String[] args) throws Exception {
        if (args.length == 0 || args[0].equals("--maps")) {
            dumpMaps();
        } else if (args[0].equals("--corpus")) {
            dumpCorpus();
        } else {
            throw new IllegalArgumentException("unknown mode: " + args[0]);
        }
    }

    private static void dumpMaps() {
        // Kotlin/JVM String.startsWith(ignoreCase = true) delegates to
        // regionMatches, whose primitive case relation uses code points.
        // Dump the complete scalar domain sparsely, with identity as the
        // specified default, so supplementary mappings are not lost.
        for (int codePoint = Character.MIN_CODE_POINT;
                codePoint <= Character.MAX_CODE_POINT;
                codePoint++) {
            int uppercase = Character.toUpperCase(codePoint);
            int lowercase = Character.toLowerCase(codePoint);
            if (uppercase != codePoint || lowercase != codePoint) {
                System.out.println(String.join("\t",
                        "SCALAR", hex(codePoint), hex(uppercase), hex(lowercase)));
            }
        }

        // Kotlin replaceFirstChar passes a UTF-16 Char to Char.titlecase().
        // Full uppercase/lowercase are therefore captured for every possible
        // UTF-16 code unit, while simple titlecase follows Character.toTitleCase.
        for (int codeUnit = Character.MIN_VALUE; codeUnit <= Character.MAX_VALUE; codeUnit++) {
            char character = (char) codeUnit;
            String input = String.valueOf(character);
            String uppercase = input.toUpperCase(Locale.ROOT);
            String lowercase = input.toLowerCase(Locale.ROOT);
            String titlecase = titlecase(character, uppercase);
            System.out.println(String.join("\t",
                    "CHAR",
                    hex(codeUnit),
                    hex(Character.toUpperCase(character)),
                    hex(Character.toLowerCase(character)),
                    hex(Character.toTitleCase(character)),
                    encode(uppercase),
                    encode(lowercase),
                    encode(titlecase)));
        }

        // ConditionalSpecialCasing's Final_Cased rule needs the Unicode
        // Cased property while lowering a complete string. Character's
        // public predicates expose the same derived property used by the
        // pinned JDK implementation: Uppercase, Lowercase, or Titlecase.
        for (int codePoint = Character.MIN_CODE_POINT;
                codePoint <= Character.MAX_CODE_POINT;
                codePoint++) {
            if (isCased(codePoint)) {
                System.out.println(String.join("\t", "CASED", hex(codePoint)));
            }
        }

        // The pinned JDK's Final_Cased implementation also consults its
        // locale-root word-boundary iterator. Capture the scalar contexts
        // that can occur between the cased letter before, and the sigma at,
        // a word's final position. This is an oracle observation of the
        // public String.lowercase contract, not runtime JDK code.
        for (int codePoint = Character.MIN_CODE_POINT;
                codePoint <= Character.MAX_CODE_POINT;
                codePoint++) {
            if (!isCased(codePoint) && isFinalSigmaContext(codePoint)) {
                System.out.println(String.join("\t", "FINAL_SIGMA", hex(codePoint)));
            }
        }
    }

    private static boolean isCased(int codePoint) {
        return Character.isLowerCase(codePoint)
                || Character.isUpperCase(codePoint)
                || Character.isTitleCase(codePoint);
    }

    private static boolean isFinalSigmaContext(int codePoint) {
        String input = "Ο" + new String(Character.toChars(codePoint)) + "Σ";
        String lowercase = input.toLowerCase(Locale.ROOT);
        return lowercase.codePointBefore(lowercase.length()) == 0x03C2;
    }

    private static void dumpCorpus() throws IOException {
        try (BufferedReader reader = new BufferedReader(
                new InputStreamReader(System.in, StandardCharsets.UTF_8))) {
            String line;
            while ((line = reader.readLine()) != null) {
                if (line.isEmpty() || line.startsWith("#")) {
                    continue;
                }
                String[] fields = line.split("\t", -1);
                switch (fields[0]) {
                    case "CAP" -> {
                        require(fields, 2);
                        System.out.println("CAP\t" + encode(capitalize(fields[1])));
                    }
                    case "START" -> {
                        require(fields, 3);
                        System.out.println("START\t" + regionMatches(fields[1], fields[2]));
                    }
                    case "LOWER" -> {
                        require(fields, 2);
                        System.out.println("LOWER\t" + encode(fields[1].toLowerCase(Locale.ROOT)));
                    }
                    default -> throw new IllegalArgumentException(
                            "unknown corpus operation: " + fields[0]);
                }
            }
        }
    }

    private static void require(String[] fields, int length) {
        if (fields.length != length) {
            throw new IllegalArgumentException("expected " + length + " tab fields");
        }
    }

    private static String capitalize(String input) {
        if (input.isEmpty()) {
            return input;
        }
        char first = input.charAt(0);
        String firstTitlecase = titlecase(first, String.valueOf(first).toUpperCase(Locale.ROOT));
        return firstTitlecase + input.substring(1);
    }

    private static String titlecase(char character, String uppercase) {
        if (uppercase.length() > 1) {
            return character == '\u0149'
                    ? uppercase
                    : uppercase.substring(0, 1)
                            + uppercase.substring(1).toLowerCase(Locale.ROOT);
        }
        return String.valueOf(Character.toTitleCase(character));
    }

    private static boolean regionMatches(String string, String prefix) {
        return string.regionMatches(true, 0, prefix, 0, prefix.length());
    }

    private static String encode(String value) {
        StringBuilder result = new StringBuilder();
        for (int index = 0; index < value.length();) {
            int codePoint = value.codePointAt(index);
            if (result.length() != 0) {
                result.append(',');
            }
            result.append(hex(codePoint));
            index += Character.charCount(codePoint);
        }
        return result.length() == 0 ? "-" : result.toString();
    }

    private static String hex(int value) {
        return String.format(Locale.ROOT, "%04X", value);
    }
}
