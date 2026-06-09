package io.weedforge;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;

/**
 * A tiny, dependency-free JSON reader — just enough to parse SeaweedFS responses and
 * the conformance vectors. Produces a value tree of {@link Map}, {@link List}, String,
 * Long, Double, Boolean, and null. Not a general-purpose library.
 */
final class Json {

    private final String s;
    private int i;

    private Json(String s) { this.s = s; }

    static Object parse(String text) {
        Json p = new Json(text);
        p.skipWs();
        Object v = p.readValue();
        p.skipWs();
        if (p.i < p.s.length()) {
            throw new IllegalArgumentException("trailing content at index " + p.i);
        }
        return v;
    }

    // --- typed convenience accessors over a parsed object ---

    @SuppressWarnings("unchecked")
    static Map<String, Object> asObject(Object o) {
        if (!(o instanceof Map)) {
            throw new IllegalArgumentException("expected JSON object");
        }
        return (Map<String, Object>) o;
    }

    static String optString(Map<String, Object> obj, String key) {
        Object v = obj.get(key);
        return v == null ? null : String.valueOf(v);
    }

    static long optLong(Map<String, Object> obj, String key, long dflt) {
        Object v = obj.get(key);
        if (v instanceof Number) return ((Number) v).longValue();
        if (v instanceof String) {
            try { return Long.parseLong((String) v); } catch (NumberFormatException e) { return dflt; }
        }
        return dflt;
    }

    @SuppressWarnings("unchecked")
    static List<Object> optArray(Map<String, Object> obj, String key) {
        Object v = obj.get(key);
        return v instanceof List ? (List<Object>) v : null;
    }

    // --- parser core ---

    private Object readValue() {
        char c = peek();
        switch (c) {
            case '{': return readObject();
            case '[': return readArray();
            case '"': return readString();
            case 't': case 'f': return readBool();
            case 'n': return readNull();
            default: return readNumber();
        }
    }

    private Map<String, Object> readObject() {
        Map<String, Object> obj = new LinkedHashMap<String, Object>();
        expect('{');
        skipWs();
        if (peek() == '}') { i++; return obj; }
        while (true) {
            skipWs();
            String key = readString();
            skipWs();
            expect(':');
            skipWs();
            obj.put(key, readValue());
            skipWs();
            char c = next();
            if (c == '}') break;
            if (c != ',') throw err("expected ',' or '}'");
        }
        return obj;
    }

    private List<Object> readArray() {
        List<Object> arr = new ArrayList<Object>();
        expect('[');
        skipWs();
        if (peek() == ']') { i++; return arr; }
        while (true) {
            skipWs();
            arr.add(readValue());
            skipWs();
            char c = next();
            if (c == ']') break;
            if (c != ',') throw err("expected ',' or ']'");
        }
        return arr;
    }

    private String readString() {
        expect('"');
        StringBuilder sb = new StringBuilder();
        while (true) {
            char c = next();
            if (c == '"') break;
            if (c == '\\') {
                char e = next();
                switch (e) {
                    case '"': sb.append('"'); break;
                    case '\\': sb.append('\\'); break;
                    case '/': sb.append('/'); break;
                    case 'b': sb.append('\b'); break;
                    case 'f': sb.append('\f'); break;
                    case 'n': sb.append('\n'); break;
                    case 'r': sb.append('\r'); break;
                    case 't': sb.append('\t'); break;
                    case 'u':
                        sb.append((char) Integer.parseInt(s.substring(i, i + 4), 16));
                        i += 4;
                        break;
                    default: throw err("invalid escape \\" + e);
                }
            } else {
                sb.append(c);
            }
        }
        return sb.toString();
    }

    private Object readNumber() {
        int start = i;
        boolean floating = false;
        while (i < s.length()) {
            char c = s.charAt(i);
            if (c == '-' || c == '+' || (c >= '0' && c <= '9')) {
                i++;
            } else if (c == '.' || c == 'e' || c == 'E') {
                floating = true;
                i++;
            } else {
                break;
            }
        }
        String num = s.substring(start, i);
        if (num.isEmpty()) throw err("invalid number");
        if (floating) {
            return Double.valueOf(Double.parseDouble(num));
        }
        try {
            return Long.valueOf(Long.parseLong(num));
        } catch (NumberFormatException e) {
            return Double.valueOf(Double.parseDouble(num));
        }
    }

    private Boolean readBool() {
        if (s.startsWith("true", i)) { i += 4; return Boolean.TRUE; }
        if (s.startsWith("false", i)) { i += 5; return Boolean.FALSE; }
        throw err("invalid literal");
    }

    private Object readNull() {
        if (s.startsWith("null", i)) { i += 4; return null; }
        throw err("invalid literal");
    }

    private char peek() {
        if (i >= s.length()) throw err("unexpected end of input");
        return s.charAt(i);
    }

    private char next() {
        if (i >= s.length()) throw err("unexpected end of input");
        return s.charAt(i++);
    }

    private void expect(char c) {
        char got = next();
        if (got != c) throw err("expected '" + c + "' but got '" + got + "'");
    }

    private void skipWs() {
        while (i < s.length()) {
            char c = s.charAt(i);
            if (c == ' ' || c == '\t' || c == '\n' || c == '\r') i++;
            else break;
        }
    }

    private IllegalArgumentException err(String msg) {
        return new IllegalArgumentException("JSON parse error at index " + i + ": " + msg);
    }
}
