package io.weedforge;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;

/** Builds a {@code multipart/form-data} body with a single {@code file} part. */
final class Multipart {

    final String contentType;
    final byte[] body;

    private Multipart(String contentType, byte[] body) {
        this.contentType = contentType;
        this.body = body;
    }

    static Multipart singleFile(byte[] data, String filename, String partContentType) {
        String boundary = "----weedforgeBoundary" + Long.toHexString(System.nanoTime());
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        writeAscii(out, "--" + boundary + "\r\n");
        writeAscii(out, "Content-Disposition: form-data; name=\"file\"; filename=\""
                + escapeQuotes(filename) + "\"\r\n");
        writeAscii(out, "Content-Type: " + partContentType + "\r\n\r\n");
        if (data != null) {
            out.write(data, 0, data.length);
        }
        writeAscii(out, "\r\n--" + boundary + "--\r\n");
        return new Multipart("multipart/form-data; boundary=" + boundary, out.toByteArray());
    }

    private static void writeAscii(ByteArrayOutputStream out, String s) {
        byte[] b = s.getBytes(StandardCharsets.UTF_8);
        out.write(b, 0, b.length);
    }

    private static String escapeQuotes(String s) {
        return s.replace("\\", "\\\\").replace("\"", "\\\"");
    }
}
