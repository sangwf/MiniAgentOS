#include <stdint.h>

static int32_t g_last_x509_err = 0;
static int32_t g_last_curve = 0;
static uint32_t g_cert_hslen = 0;
static uint32_t g_cert_list_len = 0;
static uint8_t g_cert_list_hi = 0;
static uint32_t g_cert_prefix0 = 0;
static uint32_t g_cert_prefix1 = 0;
static uint8_t g_cert_dump[32];
static uint8_t g_cert_dump_len = 0;
static int32_t g_skx_err = 0;
static int32_t g_skx_ret = 0;

void minios_mbedtls_set_x509_err(int32_t v) {
    g_last_x509_err = v;
}

void minios_mbedtls_set_curve(int32_t v) {
    g_last_curve = v;
}

void minios_mbedtls_set_skx_err(int32_t v) {
    g_skx_err = v;
}

void minios_mbedtls_set_skx_ret(int32_t v) {
    g_skx_ret = v;
}

void minios_mbedtls_set_cert_len(uint32_t hslen, uint32_t list_len, uint8_t list_hi) {
    g_cert_hslen = hslen;
    g_cert_list_len = list_len;
    g_cert_list_hi = list_hi;
}

void minios_mbedtls_set_cert_prefix(const uint8_t *buf) {
    if (!buf) {
        g_cert_prefix0 = 0;
        g_cert_prefix1 = 0;
        return;
    }
    g_cert_prefix0 = ((uint32_t) buf[0] << 24)
                   | ((uint32_t) buf[1] << 16)
                   | ((uint32_t) buf[2] << 8)
                   | (uint32_t) buf[3];
    g_cert_prefix1 = ((uint32_t) buf[4] << 24)
                   | ((uint32_t) buf[5] << 16)
                   | ((uint32_t) buf[6] << 8)
                   | (uint32_t) buf[7];
}

void minios_mbedtls_set_cert_dump(const uint8_t *buf, uint32_t len) {
    uint32_t i = 0;
    g_cert_dump_len = 0;
    while (i < 32) {
        g_cert_dump[i] = 0;
        i++;
    }
    if (!buf || len == 0) {
        return;
    }
    uint32_t n = len;
    if (n > 32) {
        n = 32;
    }
    i = 0;
    while (i < n) {
        g_cert_dump[i] = buf[i];
        i++;
    }
    g_cert_dump_len = (uint8_t) n;
}

int32_t minios_mbedtls_get_x509_err(void) {
    return g_last_x509_err;
}

int32_t minios_mbedtls_get_curve(void) {
    return g_last_curve;
}

int32_t minios_mbedtls_get_skx_err(void) {
    return g_skx_err;
}

int32_t minios_mbedtls_get_skx_ret(void) {
    return g_skx_ret;
}

uint32_t minios_mbedtls_get_cert_hslen(void) {
    return g_cert_hslen;
}

uint32_t minios_mbedtls_get_cert_list_len(void) {
    return g_cert_list_len;
}

uint8_t minios_mbedtls_get_cert_list_hi(void) {
    return g_cert_list_hi;
}

uint32_t minios_mbedtls_get_cert_prefix0(void) {
    return g_cert_prefix0;
}

uint32_t minios_mbedtls_get_cert_prefix1(void) {
    return g_cert_prefix1;
}

uint32_t minios_mbedtls_get_cert_dump_len(void) {
    return g_cert_dump_len;
}

uint32_t minios_mbedtls_get_cert_dump_word(uint32_t idx) {
    uint32_t base = idx * 4;
    if (base + 3 >= 32) {
        return 0;
    }
    return ((uint32_t) g_cert_dump[base] << 24)
         | ((uint32_t) g_cert_dump[base + 1] << 16)
         | ((uint32_t) g_cert_dump[base + 2] << 8)
         | (uint32_t) g_cert_dump[base + 3];
}

void minios_mbedtls_clear_diag(void) {
    g_last_x509_err = 0;
    g_last_curve = 0;
    g_skx_err = 0;
    g_skx_ret = 0;
    g_cert_hslen = 0;
    g_cert_list_len = 0;
    g_cert_list_hi = 0;
    g_cert_prefix0 = 0;
    g_cert_prefix1 = 0;
    g_cert_dump_len = 0;
    for (uint32_t i = 0; i < 32; i++) {
        g_cert_dump[i] = 0;
    }
}
