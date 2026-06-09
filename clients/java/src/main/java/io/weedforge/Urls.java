package io.weedforge;

/** URL normalization helpers shared by the volume client and the public-URL builder. */
final class Urls {

    private Urls() { }

    /** Trims a trailing slash from a master base URL. */
    static String normalizeMaster(String u) {
        return u.endsWith("/") ? u.substring(0, u.length() - 1) : u;
    }

    /**
     * Trims a trailing slash and prepends {@code http://} when the volume location has
     * no scheme (SeaweedFS returns bare {@code host:port}). This is the fix for the Rust
     * core's scheme-less public_url defect, shared by both consumers.
     */
    static String normalizeVolume(String u) {
        if (u.endsWith("/")) {
            u = u.substring(0, u.length() - 1);
        }
        if (!u.startsWith("http://") && !u.startsWith("https://")) {
            return "http://" + u;
        }
        return u;
    }
}
