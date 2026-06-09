package io.weedforge;

import java.io.IOException;
import java.util.HashMap;
import java.util.Map;

/**
 * The minimal HTTP seam the client depends on. The default implementation is
 * {@link HttpUrlConnectionTransport} (zero dependencies); advanced users can supply an
 * OkHttp/Apache-backed implementation via {@link WeedClient.Builder#transport}. Tests
 * inject fakes to exercise failover/parsing without a live server.
 */
public interface HttpTransport {

    Response send(Request request) throws IOException;

    /** An outbound HTTP request. */
    final class Request {
        public final String method;
        public final String url;
        public final Map<String, String> headers;
        public final byte[] body; // nullable

        public Request(String method, String url, Map<String, String> headers, byte[] body) {
            this.method = method;
            this.url = url;
            this.headers = headers != null ? headers : new HashMap<String, String>();
            this.body = body;
        }

        public static Request get(String url) {
            return new Request("GET", url, new HashMap<String, String>(), null);
        }

        public static Request delete(String url) {
            return new Request("DELETE", url, new HashMap<String, String>(), null);
        }
    }

    /** An inbound HTTP response. */
    final class Response {
        public final int statusCode;
        public final byte[] body;

        public Response(int statusCode, byte[] body) {
            this.statusCode = statusCode;
            this.body = body != null ? body : new byte[0];
        }

        public String bodyAsString() {
            return new String(body, java.nio.charset.StandardCharsets.UTF_8);
        }
    }
}
