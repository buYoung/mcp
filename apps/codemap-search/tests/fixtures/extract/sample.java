/** Fixture exercising Java branch-sensitive extraction. */
public class Sample {

    /** Fetch the current value. */
    public String fetch() {
        String label = "fetched value";
        return label;
    }

    /** Old fetcher. */
    @Deprecated
    public String legacyFetch() {
        return "legacy";
    }

    String packagePrivateHelper() {
        return "package-private";
    }

    @Test
    public void testFetch() {
        new Sample().fetch();
    }
}
