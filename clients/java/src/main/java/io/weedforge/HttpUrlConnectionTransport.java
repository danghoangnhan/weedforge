package io.weedforge;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.io.OutputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.security.SecureRandom;
import java.security.cert.X509Certificate;
import java.util.Map;
import javax.net.ssl.HostnameVerifier;
import javax.net.ssl.HttpsURLConnection;
import javax.net.ssl.SSLContext;
import javax.net.ssl.SSLSession;
import javax.net.ssl.SSLSocketFactory;
import javax.net.ssl.TrustManager;
import javax.net.ssl.X509TrustManager;

/**
 * Zero-dependency {@link HttpTransport} backed by {@link java.net.HttpURLConnection}.
 * JDK 8 compatible. Applies connect/read timeouts, an optional response-size cap
 * (memory-exhaustion guard), and an opt-in insecure-TLS mode.
 */
public final class HttpUrlConnectionTransport implements HttpTransport {

    private final int connectTimeoutMillis;
    private final int readTimeoutMillis;
    private final long maxResponseBytes; // 0 = unlimited
    private final SSLSocketFactory insecureSocketFactory; // null unless insecure mode

    public HttpUrlConnectionTransport(HttpClientConfig cfg) {
        this.connectTimeoutMillis = cfg.connectTimeoutMillis;
        this.readTimeoutMillis = cfg.requestTimeoutMillis;
        this.maxResponseBytes = cfg.maxDownloadBytes;
        this.insecureSocketFactory = cfg.insecureSkipVerify ? buildInsecureFactory() : null;
    }

    @Override
    public Response send(Request request) throws IOException {
        URL url = new URL(request.url);
        HttpURLConnection conn = (HttpURLConnection) url.openConnection();
        if (insecureSocketFactory != null && conn instanceof HttpsURLConnection) {
            HttpsURLConnection https = (HttpsURLConnection) conn;
            https.setSSLSocketFactory(insecureSocketFactory);
            https.setHostnameVerifier(ALLOW_ALL);
        }
        conn.setConnectTimeout(connectTimeoutMillis);
        conn.setReadTimeout(readTimeoutMillis);
        conn.setRequestMethod(request.method);
        conn.setInstanceFollowRedirects(false); // do not follow redirects (SSRF-safety)
        for (Map.Entry<String, String> e : request.headers.entrySet()) {
            conn.setRequestProperty(e.getKey(), e.getValue());
        }

        try {
            if (request.body != null) {
                conn.setDoOutput(true);
                conn.setFixedLengthStreamingMode(request.body.length);
                OutputStream os = conn.getOutputStream();
                try {
                    os.write(request.body);
                } finally {
                    os.close();
                }
            }

            int status = conn.getResponseCode();
            InputStream in = (status >= 200 && status < 400) ? conn.getInputStream() : conn.getErrorStream();
            byte[] body = readBody(in);
            return new Response(status, body);
        } finally {
            conn.disconnect();
        }
    }

    private byte[] readBody(InputStream in) throws IOException {
        if (in == null) {
            return new byte[0];
        }
        ByteArrayOutputStream out = new ByteArrayOutputStream();
        byte[] chunk = new byte[8192];
        long total = 0;
        try {
            int n;
            while ((n = in.read(chunk)) != -1) {
                total += n;
                if (maxResponseBytes > 0 && total > maxResponseBytes) {
                    throw new IOException("response body exceeds limit of " + maxResponseBytes + " bytes");
                }
                out.write(chunk, 0, n);
            }
        } finally {
            in.close();
        }
        return out.toByteArray();
    }

    private static final HostnameVerifier ALLOW_ALL = new HostnameVerifier() {
        @Override
        public boolean verify(String hostname, SSLSession session) {
            return true;
        }
    };

    private static SSLSocketFactory buildInsecureFactory() {
        try {
            TrustManager[] trustAll = new TrustManager[]{
                new X509TrustManager() {
                    @Override public void checkClientTrusted(X509Certificate[] c, String a) { }
                    @Override public void checkServerTrusted(X509Certificate[] c, String a) { }
                    @Override public X509Certificate[] getAcceptedIssuers() { return new X509Certificate[0]; }
                }
            };
            SSLContext ctx = SSLContext.getInstance("TLS");
            ctx.init(null, trustAll, new SecureRandom());
            return ctx.getSocketFactory();
        } catch (Exception e) {
            throw new WeedException.Configuration("failed to build insecure TLS factory: " + e.getMessage());
        }
    }
}
