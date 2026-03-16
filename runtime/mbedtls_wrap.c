#include "mbedtls/base64.h"
#include "mbedtls/ctr_drbg.h"
#include "mbedtls/ecp.h"
#include "mbedtls/entropy.h"
#include "mbedtls/md.h"
#include "mbedtls/platform.h"
#include "mbedtls/ssl.h"
#include "mbedtls/x509_crt.h"
#include <stddef.h>
#include <stdint.h>

extern int minios_entropy_fill(unsigned char *out, size_t len);
int32_t minios_mbedtls_get_x509_err(void);
int32_t minios_mbedtls_get_curve(void);
int32_t minios_mbedtls_get_skx_err(void);
int32_t minios_mbedtls_get_skx_ret(void);
uint32_t minios_mbedtls_get_cert_hslen(void);
uint32_t minios_mbedtls_get_cert_list_len(void);
uint8_t minios_mbedtls_get_cert_list_hi(void);
uint32_t minios_mbedtls_get_cert_prefix0(void);
uint32_t minios_mbedtls_get_cert_prefix1(void);
uint32_t minios_mbedtls_get_cert_dump_len(void);
uint32_t minios_mbedtls_get_cert_dump_word(uint32_t idx);
void minios_mbedtls_clear_diag(void);

static mbedtls_ssl_context ssl_ctx;
static mbedtls_ssl_config ssl_conf;
static mbedtls_entropy_context ssl_entropy;
static mbedtls_ctr_drbg_context ssl_ctr_drbg;
#if defined(MBEDTLS_ECP_C)
static const mbedtls_ecp_group_id ssl_curves[] = {
#if defined(MBEDTLS_ECP_DP_CURVE25519_ENABLED)
    MBEDTLS_ECP_DP_CURVE25519,
#endif
#if defined(MBEDTLS_ECP_DP_SECP256R1_ENABLED)
    MBEDTLS_ECP_DP_SECP256R1,
#endif
#if defined(MBEDTLS_ECP_DP_SECP384R1_ENABLED)
    MBEDTLS_ECP_DP_SECP384R1,
#endif
    MBEDTLS_ECP_DP_NONE,
};
#endif

static int minios_entropy_source(void *data, unsigned char *output, size_t len, size_t *olen) {
    (void)data;
    if (minios_entropy_fill(output, len) != 0) {
        return MBEDTLS_ERR_ENTROPY_SOURCE_FAILED;
    }
    if (olen) {
        *olen = len;
    }
    return 0;
}

int minios_tls_init(void) {
    const char *pers = "minios";
    mbedtls_ssl_init(&ssl_ctx);
    mbedtls_ssl_config_init(&ssl_conf);
    mbedtls_entropy_init(&ssl_entropy);
    mbedtls_ctr_drbg_init(&ssl_ctr_drbg);

    int ret = mbedtls_entropy_add_source(
        &ssl_entropy,
        minios_entropy_source,
        NULL,
        32,
        MBEDTLS_ENTROPY_SOURCE_STRONG);
    if (ret != 0) {
        return ret;
    }

    ret = mbedtls_ctr_drbg_seed(
        &ssl_ctr_drbg,
        mbedtls_entropy_func,
        &ssl_entropy,
        (const unsigned char *)pers,
        6);
    if (ret != 0) {
        return ret;
    }

    ret = mbedtls_ssl_config_defaults(
        &ssl_conf,
        MBEDTLS_SSL_IS_CLIENT,
        MBEDTLS_SSL_TRANSPORT_STREAM,
        MBEDTLS_SSL_PRESET_DEFAULT);
    if (ret != 0) {
        return ret;
    }

    mbedtls_ssl_conf_authmode(&ssl_conf, MBEDTLS_SSL_VERIFY_NONE);
#if defined(MBEDTLS_ECP_C)
    mbedtls_ssl_conf_curves(&ssl_conf, ssl_curves);
#endif
    mbedtls_ssl_conf_rng(&ssl_conf, mbedtls_ctr_drbg_random, &ssl_ctr_drbg);

    ret = mbedtls_ssl_setup(&ssl_ctx, &ssl_conf);
    if (ret != 0) {
        return ret;
    }
    return 0;
}

int minios_tls_reset(void) {
    return mbedtls_ssl_session_reset(&ssl_ctx);
}

int minios_tls_last_x509_err(void) {
    return (int) minios_mbedtls_get_x509_err();
}

int minios_tls_last_curve(void) {
    return (int) minios_mbedtls_get_curve();
}

int minios_tls_last_skx_err(void) {
    return (int) minios_mbedtls_get_skx_err();
}

int minios_tls_last_skx_ret(void) {
    return (int) minios_mbedtls_get_skx_ret();
}

uint32_t minios_tls_cert_hslen(void) {
    return minios_mbedtls_get_cert_hslen();
}

uint32_t minios_tls_cert_list_len(void) {
    return minios_mbedtls_get_cert_list_len();
}

uint8_t minios_tls_cert_list_hi(void) {
    return minios_mbedtls_get_cert_list_hi();
}

uint32_t minios_tls_cert_prefix0(void) {
    return minios_mbedtls_get_cert_prefix0();
}

uint32_t minios_tls_cert_prefix1(void) {
    return minios_mbedtls_get_cert_prefix1();
}

uint32_t minios_tls_cert_dump_len(void) {
    return minios_mbedtls_get_cert_dump_len();
}

uint32_t minios_tls_cert_dump_word(uint32_t idx) {
    return minios_mbedtls_get_cert_dump_word(idx);
}

void minios_tls_diag_clear(void) {
    minios_mbedtls_clear_diag();
}

void minios_tls_set_bio(void *ctx, mbedtls_ssl_send_t f_send, mbedtls_ssl_recv_t f_recv) {
    mbedtls_ssl_set_bio(&ssl_ctx, ctx, f_send, f_recv, NULL);
}

int minios_tls_set_hostname(const char *hostname) {
    return mbedtls_ssl_set_hostname(&ssl_ctx, hostname);
}

int minios_tls_handshake(void) {
    return mbedtls_ssl_handshake(&ssl_ctx);
}

int minios_tls_write(const unsigned char *buf, size_t len) {
    return mbedtls_ssl_write(&ssl_ctx, buf, len);
}

int minios_tls_read(unsigned char *buf, size_t len) {
    return mbedtls_ssl_read(&ssl_ctx, buf, len);
}

int minios_hmac_sha1(
    const unsigned char *key,
    size_t key_len,
    const unsigned char *msg,
    size_t msg_len,
    unsigned char *out20) {
    const mbedtls_md_info_t *info = mbedtls_md_info_from_type(MBEDTLS_MD_SHA1);
    if (!info) {
        return -1;
    }
    return mbedtls_md_hmac(info, key, key_len, msg, msg_len, out20);
}

int minios_base64_encode(
    const unsigned char *src,
    size_t slen,
    unsigned char *dst,
    size_t dlen,
    size_t *out_len) {
    return mbedtls_base64_encode(dst, dlen, out_len, src, slen);
}
