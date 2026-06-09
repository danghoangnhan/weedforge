package io.weedforge;

/** Image transformation parameters appended to a public URL. Uses a fluent builder. */
public final class ImageParams {

    final Integer width;   // null = unset
    final Integer height;  // null = unset
    final ResizeMode mode; // null = unset

    private ImageParams(Integer width, Integer height, ResizeMode mode) {
        this.width = width;
        this.height = height;
        this.mode = mode;
    }

    public static ImageParams dimensions(int width, int height) {
        return new ImageParams(width, height, null);
    }

    public static ImageParams width(int width) {
        return new ImageParams(width, null, null);
    }

    public static ImageParams height(int height) {
        return new ImageParams(null, height, null);
    }

    public ImageParams withMode(ResizeMode mode) {
        return new ImageParams(this.width, this.height, mode);
    }
}
