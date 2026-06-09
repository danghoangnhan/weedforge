package io.weedforge;

import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;
import static org.junit.Assert.fail;

import java.nio.charset.StandardCharsets;
import java.util.concurrent.atomic.AtomicInteger;
import org.junit.Test;

/** Exercises HA failover, query encoding, scheme handling, and read via an injected fake transport. */
public class HttpClientTest {

    private static final String ASSIGN_OK =
            "{\"fid\":\"3,01637037d6\",\"url\":\"vol1:8080\",\"publicUrl\":\"pub1:8080\",\"count\":1}";
    private static final String UPLOAD_OK = "{\"size\":13,\"eTag\":\"abc\"}";

    /** A fake transport that routes by URL substring. */
    private static HttpTransport fake(final Router router) {
        return new HttpTransport() {
            @Override
            public Response send(Request req) {
                return router.route(req);
            }
        };
    }

    private interface Router {
        HttpTransport.Response route(HttpTransport.Request req);
    }

    private static HttpTransport.Response ok(String body) {
        return new HttpTransport.Response(200, body.getBytes(StandardCharsets.UTF_8));
    }

    private static HttpTransport.Response status(int code, String body) {
        return new HttpTransport.Response(code, body.getBytes(StandardCharsets.UTF_8));
    }

    @Test
    public void assignFailsOverToSecondMaster() {
        final AtomicInteger first = new AtomicInteger();
        final AtomicInteger second = new AtomicInteger();
        HttpTransport t = fake(new Router() {
            @Override
            public HttpTransport.Response route(HttpTransport.Request req) {
                if (req.url.contains("/dir/assign")) {
                    if (req.url.contains("master1")) {
                        first.incrementAndGet();
                        return status(500, "boom");
                    }
                    second.incrementAndGet();
                    return ok(ASSIGN_OK);
                }
                return ok(UPLOAD_OK); // volume upload
            }
        });

        WeedClient c = WeedClient.builder()
                .masterUrl("http://master1:9333")
                .masterUrl("http://master2:9333")
                .strategy(MasterSelectionStrategy.FAILOVER)
                .transport(t)
                .build();

        FileId fid = c.write("hello seaweed".getBytes(StandardCharsets.UTF_8), "x.txt");
        assertEquals(3L, fid.volumeId());
        assertEquals(1L, fid.fileKey());
        assertTrue("expected both masters tried", first.get() > 0 && second.get() > 0);
    }

    @Test
    public void maxRetriesZeroStillTriesOnce() {
        final AtomicInteger assigns = new AtomicInteger();
        HttpTransport t = fake(new Router() {
            @Override
            public HttpTransport.Response route(HttpTransport.Request req) {
                if (req.url.contains("/dir/assign")) {
                    assigns.incrementAndGet();
                    return ok(ASSIGN_OK);
                }
                return ok(UPLOAD_OK);
            }
        });
        WeedClient c = WeedClient.builder()
                .masterUrl("http://master1:9333")
                .maxRetries(0) // would mean "never try" in the buggy Rust core
                .transport(t)
                .build();
        c.write("x".getBytes(StandardCharsets.UTF_8), null);
        assertEquals(1, assigns.get());
    }

    @Test
    public void publicUrlPrependsScheme() {
        HttpTransport t = fake(new Router() {
            @Override
            public HttpTransport.Response route(HttpTransport.Request req) {
                return ok("{\"volumeId\":\"3\",\"locations\":[{\"url\":\"127.0.0.1:8080\",\"publicUrl\":\"cdn.example.com\"}]}");
            }
        });
        WeedClient c = WeedClient.builder().masterUrl("http://m:9333").transport(t).build();
        FileId fid = FileId.parse("3,01637037d6");

        String url = c.publicUrl(fid);
        assertTrue("missing scheme/host: " + url, url.startsWith("http://127.0.0.1:8080/"));
        assertTrue("missing fid: " + url, url.endsWith("/3,01637037d6"));

        String resized = c.publicUrlResized(fid, 200, 100);
        assertTrue("should prefer public host: " + resized, resized.startsWith("http://cdn.example.com/"));
        assertTrue("missing params: " + resized, resized.contains("width=200") && resized.contains("height=100"));
    }

    @Test
    public void readRoundTripsThroughVolume() {
        final String payload = "hello seaweed";
        HttpTransport t = fake(new Router() {
            @Override
            public HttpTransport.Response route(HttpTransport.Request req) {
                if (req.url.contains("/dir/lookup")) {
                    return ok("{\"volumeId\":\"3\",\"locations\":[{\"url\":\"vol1:8080\"}]}");
                }
                return ok(payload);
            }
        });
        WeedClient c = WeedClient.builder().masterUrl("http://m:9333").transport(t).build();
        byte[] data = c.read(FileId.parse("3,01637037d6"));
        assertEquals(payload, new String(data, StandardCharsets.UTF_8));
    }

    @Test
    public void assignEncodesQueryParams() {
        final StringBuilder seen = new StringBuilder();
        HttpTransport t = fake(new Router() {
            @Override
            public HttpTransport.Response route(HttpTransport.Request req) {
                if (req.url.contains("/dir/assign")) {
                    seen.setLength(0);
                    seen.append(req.url);
                    return ok(ASSIGN_OK);
                }
                return ok(UPLOAD_OK);
            }
        });
        WeedClient c = WeedClient.builder().masterUrl("http://m:9333").transport(t).build();
        c.writeWithOptions("x".getBytes(StandardCharsets.UTF_8),
                new WriteOptions().collection("a&replication=999"));
        assertTrue("query not percent-encoded: " + seen,
                seen.toString().contains("collection=a%26replication%3D999"));
    }

    @Test
    public void buildRequiresMasterUrl() {
        try {
            WeedClient.builder().build();
            fail("expected configuration error");
        } catch (WeedException.Configuration expected) {
            // ok
        }
    }

    @Test
    public void downloadLimitEnforced() {
        StringBuilder big = new StringBuilder();
        for (int i = 0; i < 1000; i++) big.append('A');
        final String body = big.toString();
        HttpTransport t = fake(new Router() {
            @Override
            public HttpTransport.Response route(HttpTransport.Request req) {
                if (req.url.contains("/dir/lookup")) {
                    return ok("{\"volumeId\":\"3\",\"locations\":[{\"url\":\"vol1:8080\"}]}");
                }
                return ok(body);
            }
        });
        WeedClient c = WeedClient.builder()
                .masterUrl("http://m:9333")
                .config(new HttpClientConfig().maxDownloadBytes(100))
                .transport(t)
                .build();
        try {
            c.read(FileId.parse("3,01637037d6"));
            fail("expected download-limit error");
        } catch (WeedException.DownloadFailed expected) {
            // ok
        }
    }
}
