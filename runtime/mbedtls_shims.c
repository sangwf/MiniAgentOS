#include <stddef.h>
#include <stdarg.h>

int mbedtls_snprintf(char *s, size_t n, const char *fmt, ...) {
    (void) fmt;
    if (s && n > 0) {
        s[0] = '\0';
    }
    return 0;
}

int snprintf(char *s, size_t n, const char *fmt, ...) {
    (void) fmt;
    if (s && n > 0) {
        s[0] = '\0';
    }
    return 0;
}

int vsnprintf(char *s, size_t n, const char *fmt, va_list ap) {
    (void) fmt;
    (void) ap;
    if (s && n > 0) {
        s[0] = '\0';
    }
    return 0;
}
