package io.weedforge;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.List;
import java.util.Map;
import org.junit.Test;

/** Runs the shared cross-language FileId conformance vectors (clients/conformance). */
public class FileIdConformanceTest {

    private Map<String, Object> loadVectors() throws IOException {
        InputStream in = getClass().getResourceAsStream("/conformance/fileid_vectors.json");
        assertTrue("conformance vectors not on classpath", in != null);
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        byte[] buf = new byte[4096];
        int n;
        while ((n = in.read(buf)) != -1) {
            out.write(buf, 0, n);
        }
        in.close();
        return Json.asObject(Json.parse(new String(out.toByteArray(), StandardCharsets.UTF_8)));
    }

    @Test
    public void validVectors() throws IOException {
        Map<String, Object> root = loadVectors();
        List<Object> valid = Json.optArray(root, "valid");
        assertTrue("no valid vectors", valid != null && !valid.isEmpty());
        for (Object o : valid) {
            Map<String, Object> vec = Json.asObject(o);
            String input = Json.optString(vec, "input");
            long wantVol = Json.optLong(vec, "volume_id", -1);
            long wantKey = Long.parseUnsignedLong(Json.optString(vec, "file_key"));
            long wantCookie = Json.optLong(vec, "cookie", -1);
            String wantRender = Json.optString(vec, "render");

            FileId fid = FileId.parse(input);
            assertEquals("volumeId for " + input, wantVol, fid.volumeId());
            assertEquals("fileKey for " + input, wantKey, fid.fileKey());
            assertEquals("cookie for " + input, wantCookie, fid.cookie());
            assertEquals("render for " + input, wantRender, fid.render());

            // render is canonical and stable under re-parse.
            assertEquals("re-parse stable for " + input, fid, FileId.parse(fid.render()));
        }
    }

    @Test
    public void invalidVectors() throws IOException {
        Map<String, Object> root = loadVectors();
        List<Object> invalid = Json.optArray(root, "invalid");
        assertTrue("no invalid vectors", invalid != null && !invalid.isEmpty());
        for (Object o : invalid) {
            Map<String, Object> vec = Json.asObject(o);
            String input = Json.optString(vec, "input");
            try {
                FileId.parse(input);
                fail("expected parse to fail for " + input + " (" + Json.optString(vec, "reason") + ")");
            } catch (WeedException.InvalidFileId expected) {
                // ok
            }
        }
    }

    @Test
    public void largeNeedleKeyRoundTrips() {
        // > 2^32 needle key (the case the Rust core rejects/truncates).
        FileId fid = new FileId(7L, 0x1_0000_0001L, 0xDEADBEEFL);
        assertEquals("7,0100000001deadbeef", fid.render());
        assertEquals(fid, FileId.parse("7,0100000001deadbeef"));
    }
}
