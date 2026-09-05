/*
 * Generation-only word-boundary oracle for the exact pinned Eclipse Temurin 25 runtime.
 *
 * This independently authored helper observes the runtime BreakIterator through
 * reflection only during generation. Nothing here is linked into Arkst runtime code.
 */

import java.lang.reflect.Field;
import java.lang.reflect.Method;
import java.text.BreakIterator;
import java.util.Locale;

final class DumpWordBreakJdk25 {
    private static final int MIN_CODE_POINT = Character.MIN_CODE_POINT;
    private static final int MAX_CODE_POINT = Character.MAX_CODE_POINT;

    private DumpWordBreakJdk25() {}

    public static void main(String[] args) throws Exception {
        BreakIterator iterator = BreakIterator.getWordInstance(Locale.ROOT);
        Class<?> ruleClass = Class.forName("sun.text.RuleBasedBreakIterator");
        if (!ruleClass.isInstance(iterator)) {
            throw new IllegalStateException("unexpected root word BreakIterator: " + iterator.getClass());
        }

        short[] stateTable = (short[]) field(ruleClass, "stateTable").get(iterator);
        boolean[] endStates = (boolean[]) field(ruleClass, "endStates").get(iterator);
        boolean[] lookaheadStates = (boolean[]) field(ruleClass, "lookaheadStates").get(iterator);
        int numCategories = field(ruleClass, "numCategories").getInt(iterator);
        Method lookupCategory = ruleClass.getDeclaredMethod("lookupCategory", int.class);
        lookupCategory.setAccessible(true);

        if (stateTable.length != endStates.length * numCategories
                || lookaheadStates.length != endStates.length) {
            throw new IllegalStateException("inconsistent root word-break tables");
        }

        System.out.println(String.join("\t",
                "META",
                Integer.toString(numCategories),
                Integer.toString(endStates.length),
                Integer.toString(stateTable.length),
                iterator.getClass().getName()));
        for (int index = 0; index < stateTable.length; index++) {
            System.out.println("STATE\t" + index + "\t" + stateTable[index]);
        }
        for (int index = 0; index < endStates.length; index++) {
            System.out.println("END\t" + index + "\t" + (endStates[index] ? "1" : "0"));
            System.out.println("LOOK\t" + index + "\t" + (lookaheadStates[index] ? "1" : "0"));
        }

        int rangeStart = -1;
        int rangeEnd = -1;
        int rangeCategory = Integer.MIN_VALUE;
        for (int codePoint = MIN_CODE_POINT; codePoint <= MAX_CODE_POINT; codePoint++) {
            if (codePoint >= Character.MIN_SURROGATE && codePoint <= Character.MAX_SURROGATE) {
                if (rangeStart >= 0) {
                    emitRange(rangeStart, rangeEnd, rangeCategory);
                    rangeStart = -1;
                }
                continue;
            }
            int category = (Integer) lookupCategory.invoke(iterator, codePoint);
            if (rangeStart >= 0 && codePoint == rangeEnd + 1 && category == rangeCategory) {
                rangeEnd = codePoint;
            } else {
                if (rangeStart >= 0) {
                    emitRange(rangeStart, rangeEnd, rangeCategory);
                }
                rangeStart = rangeEnd = codePoint;
                rangeCategory = category;
            }
        }
        if (rangeStart >= 0) {
            emitRange(rangeStart, rangeEnd, rangeCategory);
        }
    }

    private static Field field(Class<?> type, String name) throws Exception {
        Field field = type.getDeclaredField(name);
        field.setAccessible(true);
        return field;
    }

    private static void emitRange(int start, int end, int category) {
        System.out.printf(Locale.ROOT, "CAT\t%04X\t%04X\t%d%n", start, end, category);
    }
}
