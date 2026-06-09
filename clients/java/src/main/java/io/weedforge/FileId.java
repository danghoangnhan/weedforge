package io.weedforge;

/**
 * A SeaweedFS file identifier: a first-class value, not an opaque string.
 *
 * <p>The wire form is {@code {volumeId},{hex}} where {@code hex} encodes the 12-byte
 * big-endian buffer {@code [needleKey(8) | cookie(4)]} with leading zero BYTES stripped.
 * The needle key is a full 64-bit value (NOT 32-bit), so this type represents the entire
 * SeaweedFS fid space. All three components are stored as {@code long} and treated as
 * unsigned bit patterns (Java has no unsigned primitives).
 */
public final class FileId {

    private static final int COOKIE_HEX_LEN = 8;   // cookie = 4 bytes -> 8 hex chars
    private static final int MAX_KC_HEX_LEN = 24;  // 8-byte key + 4-byte cookie

    private final long volumeId; // unsigned u32
    private final long fileKey;  // unsigned u64 bit pattern
    private final long cookie;   // unsigned u32

    public FileId(long volumeId, long fileKey, long cookie) {
        this.volumeId = volumeId;
        this.fileKey = fileKey;
        this.cookie = cookie;
    }

    /** @return the volume id (unsigned 32-bit, held in a long). */
    public long volumeId() { return volumeId; }

    /** @return the needle/file key (unsigned 64-bit bit pattern). */
    public long fileKey() { return fileKey; }

    /** @return the cookie (unsigned 32-bit, held in a long). */
    public long cookie() { return cookie; }

    /**
     * Parses a SeaweedFS fid string. Mirrors SeaweedFS's own parser: the last 8 hex
     * characters are the cookie, and the remaining prefix is the (up to 64-bit) needle key.
     *
     * @throws WeedException.InvalidFileId if the string is malformed.
     */
    public static FileId parse(String s) {
        int comma = s.indexOf(',');
        if (comma < 0) {
            throw new WeedException.InvalidFileId(s, "missing comma separator");
        }
        String volStr = s.substring(0, comma);
        String kc = s.substring(comma + 1);

        if (!isAllDigits(volStr)) {
            throw new WeedException.InvalidFileId(s, "invalid volume id");
        }
        long vol;
        try {
            vol = Long.parseLong(volStr);
        } catch (NumberFormatException e) {
            throw new WeedException.InvalidFileId(s, "invalid volume id: " + e.getMessage());
        }
        if (vol < 0 || vol > 0xFFFFFFFFL) {
            throw new WeedException.InvalidFileId(s, "volume id out of range");
        }

        // SeaweedFS requires len(kc) > 8 and <= 24. Rejecting all non-hex bytes here also
        // rejects a leading '+'/'-' that Long.parseUnsignedLong would otherwise accept.
        if (kc.length() <= COOKIE_HEX_LEN || kc.length() > MAX_KC_HEX_LEN) {
            throw new WeedException.InvalidFileId(s, "key/cookie hex length out of range");
        }
        if (!isAllHex(kc)) {
            throw new WeedException.InvalidFileId(s, "non-hex character in key/cookie");
        }

        int split = kc.length() - COOKIE_HEX_LEN;
        String keyHex = kc.substring(0, split);
        String cookieHex = kc.substring(split);

        long key;
        long ck;
        try {
            key = Long.parseUnsignedLong(keyHex, 16); // up to 16 hex -> full 64 bits
            ck = Long.parseLong(cookieHex, 16);       // exactly 8 hex -> fits a signed long
        } catch (NumberFormatException e) {
            throw new WeedException.InvalidFileId(s, "invalid key/cookie hex: " + e.getMessage());
        }
        return new FileId(vol, key, ck);
    }

    /**
     * Returns the canonical SeaweedFS string form (leading zero bytes stripped).
     * {@code render(parse(s))} reproduces the server's representation; {@code parse(render(f)).equals(f)}.
     */
    public String render() {
        byte[] buf = new byte[12];
        for (int i = 0; i < 8; i++) {
            buf[i] = (byte) (fileKey >>> (56 - 8 * i));
        }
        for (int i = 0; i < 4; i++) {
            buf[8 + i] = (byte) (cookie >>> (24 - 8 * i));
        }
        int start = 0;
        while (start < buf.length - 1 && buf[start] == 0) {
            start++;
        }
        StringBuilder sb = new StringBuilder(32);
        sb.append(Long.toUnsignedString(volumeId)).append(',');
        for (int i = start; i < buf.length; i++) {
            int b = buf[i] & 0xFF;
            sb.append(Character.forDigit(b >>> 4, 16));
            sb.append(Character.forDigit(b & 0xF, 16));
        }
        return sb.toString();
    }

    @Override
    public String toString() { return render(); }

    @Override
    public boolean equals(Object o) {
        if (this == o) return true;
        if (!(o instanceof FileId)) return false;
        FileId f = (FileId) o;
        return volumeId == f.volumeId && fileKey == f.fileKey && cookie == f.cookie;
    }

    @Override
    public int hashCode() {
        int result = Long.hashCode(volumeId);
        result = 31 * result + Long.hashCode(fileKey);
        result = 31 * result + Long.hashCode(cookie);
        return result;
    }

    private static boolean isAllDigits(String s) {
        if (s.isEmpty()) return false;
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            if (c < '0' || c > '9') return false;
        }
        return true;
    }

    private static boolean isAllHex(String s) {
        if (s.isEmpty()) return false;
        for (int i = 0; i < s.length(); i++) {
            char c = s.charAt(i);
            boolean hex = (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f') || (c >= 'A' && c <= 'F');
            if (!hex) return false;
        }
        return true;
    }
}
