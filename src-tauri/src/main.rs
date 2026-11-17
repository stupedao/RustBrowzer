fn main() {
    // Keep the app off the GPU where the platform allows it, and stop WebKit
    // from spawning helper processes we do not need. These must be set before
    // any UI toolkit initialization, so they live here at the very top.
    //
    // - WEBKIT_DISABLE_COMPOSITING_MODE=1 : makes WebKitGTK paint without the
    //   GPU compositor (falls back to CPU rendering).
    // - WEBKIT_DISABLE_DMABUF_RENDERER=1  : avoids the DMA-BUF renderer that
    //   requires GPU buffers.
    // - LIBGL_ALWAYS_SOFTWARE=1           : software OpenGL for anything that
    //   still insists on GL.
    // `set_var` is `unsafe` since edition 2024: it must run before any other
    // threads exist (true here — first statement of main), which is the
    // documented safe pattern.
    unsafe {
        for (k, v) in [
            ("WEBKIT_DISABLE_COMPOSITING_MODE", "1"),
            ("WEBKIT_DISABLE_DMABUF_RENDERER", "1"),
            ("LIBGL_ALWAYS_SOFTWARE", "1"),
        ] {
            // Only set when the user has not chosen a value themselves.
            if std::env::var_os(k).is_none() {
                std::env::set_var(k, v);
            }
        }
    }
    rustbrowzer_lib::run()
}
