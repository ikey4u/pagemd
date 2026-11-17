#ifndef PAGEMD_H
#define PAGEMD_H

#ifdef __cplusplus
extern "C" {
#endif

/// Renders Markdown (UTF-8) into a self-contained HTML document for a web
/// view. Returns a UTF-8 string to free with pagemd_string_free, or NULL on
/// failure (see pagemd_last_error).
char *pagemd_render_html(const char *markdown);

/// Uses `path` (UTF-8) as pagemd's cache directory, for example the app's
/// Caches directory. Returns 0 on success, -1 on failure.
int pagemd_set_cache_dir(const char *path);

/// The last failure on this thread, or NULL. Free with pagemd_string_free.
char *pagemd_last_error(void);

/// Frees a string returned by this library.
void pagemd_string_free(char *string);

#ifdef __cplusplus
}
#endif

#endif /* PAGEMD_H */
