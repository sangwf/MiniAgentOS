#include "mbedtls/aes.h"
#include "mbedtls/base64.h"
#include "mbedtls/ctr_drbg.h"
#include "mbedtls/debug.h"
#include "mbedtls/ecp.h"
#include "mbedtls/entropy.h"
#include "mbedtls/md.h"
#include "mbedtls/platform.h"
#include "mbedtls/ssl.h"
#include "mbedtls/ssl_internal.h"
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
static uint32_t ssl_export_count;
static uint64_t ssl_export_client_random_hash;
static uint64_t ssl_export_server_random_hash;
static uint64_t ssl_export_master_hash;
static uint64_t ssl_export_keyblock_hash;
static uint64_t ssl_export_client_random_prefix;
static uint64_t ssl_export_server_random_prefix;
static uint32_t ssl_export_maclen;
static uint32_t ssl_export_keylen;
static uint32_t ssl_export_ivlen;
static int ssl_export_prf_type;
static uint64_t ssl_export_client_write_key_hash;
static uint64_t ssl_export_server_write_key_hash;
static uint64_t ssl_export_client_write_key_aes_zero_hash;
static uint64_t ssl_export_client_write_key_aes_zero_hash_static;
static uint64_t ssl_export_client_write_key_prefix;
static uint64_t ssl_export_client_write_mac_hash;
static unsigned char ssl_export_client_write_mac[64];
static size_t ssl_export_client_write_mac_len;
static unsigned char ssl_export_client_write_key[32];
static size_t ssl_export_client_write_key_len;
static int ssl_last_out_record_decrypt_ok;
static uint64_t ssl_last_out_record_plaintext_hash;
static uint32_t ssl_last_out_record_plaintext_len;
static uint32_t ssl_last_out_record_padlen;
static int ssl_last_cbc_reencrypt_match;
static uint64_t ssl_last_cbc_plain_hash;
static uint64_t ssl_last_cbc_expected_cipher_hash;
static uint64_t ssl_last_cbc_actual_cipher_hash;
static uint32_t ssl_last_cbc_len;
static unsigned char ssl_last_cbc_expected_cipher[192];
static int ssl_last_mac_match;
static uint64_t ssl_last_expected_mac_hash;
static uint64_t ssl_last_actual_mac_hash;
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

static uint64_t minios_load_be64(const unsigned char *p) {
    return ((uint64_t) p[0] << 56) |
           ((uint64_t) p[1] << 48) |
           ((uint64_t) p[2] << 40) |
           ((uint64_t) p[3] << 32) |
           ((uint64_t) p[4] << 24) |
           ((uint64_t) p[5] << 16) |
           ((uint64_t) p[6] << 8) |
           ((uint64_t) p[7]);
}

static uint64_t minios_fnv1a64(const unsigned char *p, size_t len) {
    uint64_t hash = 1469598103934665603ULL;
    size_t i = 0;
    while (i < len) {
        hash ^= p[i];
        hash *= 1099511628211ULL;
        i++;
    }
    return hash;
}

static uint64_t minios_aes_zero_hash_from_key(const unsigned char *key, size_t key_len) {
    unsigned char block[16] = {0};
    unsigned char out[16];
    mbedtls_aes_context aes;
    int ret;

    if (key == NULL) {
        return 0;
    }
    if (!(key_len == 16 || key_len == 24 || key_len == 32)) {
        return 0;
    }

    mbedtls_aes_init(&aes);
    ret = mbedtls_aes_setkey_enc(&aes, key, (unsigned int) (key_len * 8));
    if (ret != 0) {
        mbedtls_aes_free(&aes);
        return 0;
    }
    ret = mbedtls_aes_crypt_ecb(&aes, MBEDTLS_AES_ENCRYPT, block, out);
    mbedtls_aes_free(&aes);
    if (ret != 0) {
        return 0;
    }
    return minios_fnv1a64(out, sizeof(out));
}

static uint64_t minios_aes_zero_hash_from_key_static(const unsigned char *key, size_t key_len) {
    static mbedtls_aes_context aes;
    unsigned char block[16] = {0};
    unsigned char out[16];
    int ret;

    if (key == NULL) {
        return 0;
    }
    if (!(key_len == 16 || key_len == 24 || key_len == 32)) {
        return 0;
    }

    mbedtls_aes_init(&aes);
    ret = mbedtls_aes_setkey_enc(&aes, key, (unsigned int) (key_len * 8));
    if (ret != 0) {
        mbedtls_aes_free(&aes);
        return 0;
    }
    ret = mbedtls_aes_crypt_ecb(&aes, MBEDTLS_AES_ENCRYPT, block, out);
    mbedtls_aes_free(&aes);
    if (ret != 0) {
        return 0;
    }
    return minios_fnv1a64(out, sizeof(out));
}

static uint64_t minios_aes256_zero_key_self_hash(void) {
    static const unsigned char zero_key[32] = {0};
    return minios_aes_zero_hash_from_key(zero_key, sizeof(zero_key));
}

static mbedtls_ssl_transform *minios_active_transform(void) {
    if (ssl_ctx.transform_out != NULL) {
        return ssl_ctx.transform_out;
    }
    return ssl_ctx.transform_negotiate;
}

#if defined(MBEDTLS_SSL_EXPORT_KEYS)
static int minios_tls_export_keys_ext(
    void *p_expkey,
    const unsigned char *ms,
    const unsigned char *kb,
    size_t maclen,
    size_t keylen,
    size_t ivlen,
    const unsigned char client_random[32],
    const unsigned char server_random[32],
    mbedtls_tls_prf_types tls_prf_type) {
    (void) p_expkey;
    size_t kb_len = 2 * maclen + 2 * keylen + 2 * ivlen;
    ssl_export_count++;
    ssl_export_client_random_hash = minios_fnv1a64(client_random, 32);
    ssl_export_server_random_hash = minios_fnv1a64(server_random, 32);
    ssl_export_master_hash = minios_fnv1a64(ms, 48);
    ssl_export_keyblock_hash = minios_fnv1a64(kb, kb_len);
    ssl_export_client_random_prefix = minios_load_be64(client_random);
    ssl_export_server_random_prefix = minios_load_be64(server_random);
    ssl_export_maclen = (uint32_t) maclen;
    ssl_export_keylen = (uint32_t) keylen;
    ssl_export_ivlen = (uint32_t) ivlen;
    ssl_export_prf_type = (int) tls_prf_type;
    if (keylen != 0) {
        const unsigned char *client_write_mac = kb;
        const unsigned char *client_write_key = kb + (maclen * 2);
        const unsigned char *server_write_key = client_write_key + keylen;
        ssl_export_client_write_mac_hash = minios_fnv1a64(client_write_mac, maclen);
        ssl_export_client_write_key_hash = minios_fnv1a64(client_write_key, keylen);
        ssl_export_server_write_key_hash = minios_fnv1a64(server_write_key, keylen);
        ssl_export_client_write_key_aes_zero_hash =
            minios_aes_zero_hash_from_key(client_write_key, keylen);
        ssl_export_client_write_key_aes_zero_hash_static =
            minios_aes_zero_hash_from_key_static(client_write_key, keylen);
        ssl_export_client_write_key_prefix =
            keylen >= 8 ? minios_load_be64(client_write_key) : 0;
        ssl_export_client_write_mac_len = maclen;
        if (ssl_export_client_write_mac_len > sizeof(ssl_export_client_write_mac)) {
            ssl_export_client_write_mac_len = sizeof(ssl_export_client_write_mac);
        }
        memcpy(ssl_export_client_write_mac, client_write_mac, ssl_export_client_write_mac_len);
        ssl_export_client_write_key_len = keylen;
        if (ssl_export_client_write_key_len > sizeof(ssl_export_client_write_key)) {
            ssl_export_client_write_key_len = sizeof(ssl_export_client_write_key);
        }
        memcpy(ssl_export_client_write_key, client_write_key, ssl_export_client_write_key_len);
    } else {
        ssl_export_client_write_mac_hash = 0;
        ssl_export_client_write_key_hash = 0;
        ssl_export_server_write_key_hash = 0;
        ssl_export_client_write_key_aes_zero_hash = 0;
        ssl_export_client_write_key_aes_zero_hash_static = 0;
        ssl_export_client_write_key_prefix = 0;
        ssl_export_client_write_mac_len = 0;
        ssl_export_client_write_key_len = 0;
    }
    return 0;
}
#endif

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
#if defined(MBEDTLS_SSL_EXPORT_KEYS)
    mbedtls_ssl_conf_export_keys_ext_cb(&ssl_conf, minios_tls_export_keys_ext, NULL);
#endif

    ret = mbedtls_ssl_setup(&ssl_ctx, &ssl_conf);
    if (ret != 0) {
        return ret;
    }
    return 0;
}

void minios_tls_free_all(void) {
    mbedtls_ssl_free(&ssl_ctx);
    mbedtls_ssl_config_free(&ssl_conf);
    mbedtls_entropy_free(&ssl_entropy);
    mbedtls_ctr_drbg_free(&ssl_ctr_drbg);
}

int minios_tls_reset(void) {
    return mbedtls_ssl_session_reset(&ssl_ctx);
}

uint32_t minios_tls_verify_result(void) {
    return mbedtls_ssl_get_verify_result(&ssl_ctx);
}

int minios_tls_state(void) {
    return ssl_ctx.state;
}

uint64_t minios_tls_cur_out_ctr(void) {
    return minios_load_be64(ssl_ctx.cur_out_ctr);
}

uint64_t minios_tls_in_ctr(void) {
    if (ssl_ctx.in_ctr == NULL) {
        return 0;
    }
    return minios_load_be64(ssl_ctx.in_ctr);
}

int minios_tls_has_transform_out(void) {
    return ssl_ctx.transform_out != NULL;
}

int minios_tls_check_pending(void) {
    return mbedtls_ssl_check_pending(&ssl_ctx);
}

int minios_tls_close_notify(void) {
    return mbedtls_ssl_close_notify(&ssl_ctx);
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

uint32_t minios_tls_export_count(void) {
    return ssl_export_count;
}

uint64_t minios_tls_export_client_random_hash(void) {
    return ssl_export_client_random_hash;
}

uint64_t minios_tls_export_server_random_hash(void) {
    return ssl_export_server_random_hash;
}

uint64_t minios_tls_export_master_hash(void) {
    return ssl_export_master_hash;
}

uint64_t minios_tls_export_keyblock_hash(void) {
    return ssl_export_keyblock_hash;
}

uint64_t minios_tls_export_client_random_prefix(void) {
    return ssl_export_client_random_prefix;
}

uint64_t minios_tls_export_server_random_prefix(void) {
    return ssl_export_server_random_prefix;
}

uint32_t minios_tls_export_maclen(void) {
    return ssl_export_maclen;
}

uint32_t minios_tls_export_keylen(void) {
    return ssl_export_keylen;
}

uint32_t minios_tls_export_ivlen(void) {
    return ssl_export_ivlen;
}

int minios_tls_export_prf_type(void) {
    return ssl_export_prf_type;
}

uint64_t minios_tls_export_client_write_key_hash(void) {
    return ssl_export_client_write_key_hash;
}

uint64_t minios_tls_export_client_write_key_prefix(void) {
    return ssl_export_client_write_key_prefix;
}

uint64_t minios_tls_export_client_write_mac_hash(void) {
    return ssl_export_client_write_mac_hash;
}

uint64_t minios_tls_export_server_write_key_hash(void) {
    return ssl_export_server_write_key_hash;
}

uint64_t minios_tls_export_client_write_key_aes_zero_hash(void) {
    return ssl_export_client_write_key_aes_zero_hash;
}

uint64_t minios_tls_export_client_write_key_aes_zero_hash_static(void) {
    return ssl_export_client_write_key_aes_zero_hash_static;
}

uint64_t minios_tls_aes256_zero_key_self_hash(void) {
    return minios_aes256_zero_key_self_hash();
}

int minios_tls_active_ciphersuite(void) {
    if (ssl_ctx.session_out != NULL) {
        return ssl_ctx.session_out->ciphersuite;
    }
    if (ssl_ctx.session_negotiate != NULL) {
        return ssl_ctx.session_negotiate->ciphersuite;
    }
    return 0;
}

int minios_tls_active_cipher_type(void) {
    mbedtls_ssl_transform *transform = minios_active_transform();
    if (transform == NULL || transform->cipher_ctx_enc.cipher_info == NULL) {
        return 0;
    }
    return (int) transform->cipher_ctx_enc.cipher_info->type;
}

int minios_tls_active_cipher_mode(void) {
    mbedtls_ssl_transform *transform = minios_active_transform();
    if (transform == NULL || transform->cipher_ctx_enc.cipher_info == NULL) {
        return 0;
    }
    return (int) transform->cipher_ctx_enc.cipher_info->mode;
}

int minios_tls_active_cipher_operation(void) {
    mbedtls_ssl_transform *transform = minios_active_transform();
    if (transform == NULL) {
        return 0;
    }
    return (int) transform->cipher_ctx_enc.operation;
}

uint32_t minios_tls_active_cipher_key_bitlen(void) {
    mbedtls_ssl_transform *transform = minios_active_transform();
    if (transform == NULL) {
        return 0;
    }
    return (uint32_t) transform->cipher_ctx_enc.key_bitlen;
}

uint64_t minios_tls_active_iv_enc_hash(void) {
    mbedtls_ssl_transform *transform = minios_active_transform();
    if (transform == NULL || transform->ivlen == 0) {
        return 0;
    }
    return minios_fnv1a64(transform->iv_enc, transform->ivlen);
}

uint64_t minios_tls_active_iv_enc_prefix(void) {
    mbedtls_ssl_transform *transform = minios_active_transform();
    if (transform == NULL || transform->ivlen < 8) {
        return 0;
    }
    return minios_load_be64(transform->iv_enc);
}

uint64_t minios_tls_active_cipher_ctx_enc_aes_zero_hash(void) {
    mbedtls_ssl_transform *transform = minios_active_transform();
    mbedtls_aes_context *aes;
    unsigned char block[16] = {0};
    unsigned char out[16];
    int ret;

    if (transform == NULL || transform->cipher_ctx_enc.cipher_info == NULL) {
        return 0;
    }
    if ((transform->cipher_ctx_enc.cipher_info->type != MBEDTLS_CIPHER_AES_128_CBC &&
         transform->cipher_ctx_enc.cipher_info->type != MBEDTLS_CIPHER_AES_192_CBC &&
         transform->cipher_ctx_enc.cipher_info->type != MBEDTLS_CIPHER_AES_256_CBC) ||
        transform->cipher_ctx_enc.cipher_info->mode != MBEDTLS_MODE_CBC ||
        transform->cipher_ctx_enc.cipher_ctx == NULL) {
        return 0;
    }

    aes = (mbedtls_aes_context *) transform->cipher_ctx_enc.cipher_ctx;
    ret = mbedtls_aes_crypt_ecb(aes, MBEDTLS_AES_ENCRYPT, block, out);
    if (ret != 0) {
        return 0;
    }
    return minios_fnv1a64(out, sizeof(out));
}

int minios_tls_analyze_outbound_record(const unsigned char *record, size_t len) {
    unsigned char iv[16];
    unsigned char plain[192];
    mbedtls_aes_context aes;
    size_t payload_len;
    size_t cipher_len;
    size_t plain_len;
    size_t i;
    int ret;
    unsigned char padlen;

    ssl_last_out_record_decrypt_ok = 0;
    ssl_last_out_record_plaintext_hash = 0;
    ssl_last_out_record_plaintext_len = 0;
    ssl_last_out_record_padlen = 0;

    if (record == NULL || len < 5 + 16 || ssl_export_client_write_key_len == 0) {
        return -1;
    }
    payload_len = ((size_t) record[3] << 8) | (size_t) record[4];
    if (payload_len + 5 > len || payload_len < 16) {
        return -1;
    }
    cipher_len = payload_len - 16;
    if (cipher_len == 0 || cipher_len > sizeof(plain) || (cipher_len % 16) != 0) {
        return -1;
    }

    memcpy(iv, record + 5, sizeof(iv));
    mbedtls_aes_init(&aes);
    ret = mbedtls_aes_setkey_dec(
        &aes,
        ssl_export_client_write_key,
        (unsigned int) (ssl_export_client_write_key_len * 8));
    if (ret != 0) {
        mbedtls_aes_free(&aes);
        return ret;
    }
    ret = mbedtls_aes_crypt_cbc(&aes, MBEDTLS_AES_DECRYPT, cipher_len, iv, record + 21, plain);
    mbedtls_aes_free(&aes);
    if (ret != 0) {
        return ret;
    }

    padlen = plain[cipher_len - 1];
    if ((size_t) padlen + 1 > cipher_len) {
        ssl_last_out_record_padlen = (uint32_t) padlen;
        return -2;
    }
    for (i = 0; i <= (size_t) padlen; i++) {
        if (plain[cipher_len - 1 - i] != padlen) {
            ssl_last_out_record_padlen = (uint32_t) padlen;
            return -3;
        }
    }

    plain_len = cipher_len - ((size_t) padlen + 1);
    ssl_last_out_record_decrypt_ok = 1;
    ssl_last_out_record_plaintext_hash = minios_fnv1a64(plain, cipher_len);
    ssl_last_out_record_plaintext_len = (uint32_t) plain_len;
    ssl_last_out_record_padlen = (uint32_t) padlen;
    return 0;
}

void minios_tls_debug_cbc_pre(
    const unsigned char *iv,
    size_t iv_len,
    const unsigned char *plain,
    size_t plain_len) {
    unsigned char iv_local[16];
    mbedtls_aes_context aes;
    int ret;

    ssl_last_cbc_reencrypt_match = 0;
    ssl_last_cbc_plain_hash = 0;
    ssl_last_cbc_expected_cipher_hash = 0;
    ssl_last_cbc_actual_cipher_hash = 0;
    ssl_last_cbc_len = 0;

    if (iv == NULL || plain == NULL || iv_len != 16 ||
        plain_len == 0 || plain_len > sizeof(ssl_last_cbc_expected_cipher) ||
        (plain_len % 16) != 0 || ssl_export_client_write_key_len == 0) {
        return;
    }

    memcpy(iv_local, iv, sizeof(iv_local));
    memcpy(ssl_last_cbc_expected_cipher, plain, plain_len);
    mbedtls_aes_init(&aes);
    ret = mbedtls_aes_setkey_enc(
        &aes,
        ssl_export_client_write_key,
        (unsigned int) (ssl_export_client_write_key_len * 8));
    if (ret != 0) {
        mbedtls_aes_free(&aes);
        return;
    }
    ret = mbedtls_aes_crypt_cbc(
        &aes,
        MBEDTLS_AES_ENCRYPT,
        plain_len,
        iv_local,
        ssl_last_cbc_expected_cipher,
        ssl_last_cbc_expected_cipher);
    mbedtls_aes_free(&aes);
    if (ret != 0) {
        return;
    }

    ssl_last_cbc_plain_hash = minios_fnv1a64(plain, plain_len);
    ssl_last_cbc_expected_cipher_hash =
        minios_fnv1a64(ssl_last_cbc_expected_cipher, plain_len);
    ssl_last_cbc_len = (uint32_t) plain_len;
}

void minios_tls_debug_cbc_post(const unsigned char *cipher, size_t cipher_len) {
    if (cipher == NULL || cipher_len == 0 || cipher_len != ssl_last_cbc_len) {
        return;
    }
    ssl_last_cbc_actual_cipher_hash = minios_fnv1a64(cipher, cipher_len);
    ssl_last_cbc_reencrypt_match =
        memcmp(ssl_last_cbc_expected_cipher, cipher, cipher_len) == 0;
}

void minios_tls_debug_mac_check(
    const unsigned char *add_data,
    size_t add_data_len,
    const unsigned char *plain,
    size_t plain_len,
    const unsigned char *actual_mac,
    size_t mac_len) {
    const mbedtls_md_info_t *md_info;
    mbedtls_md_context_t md_ctx;
    unsigned char expected[64];
    uint64_t expected_hash;
    uint64_t actual_hash;

    ssl_last_mac_match = 0;
    ssl_last_expected_mac_hash = 0;
    ssl_last_actual_mac_hash = 0;

    if (add_data == NULL || plain == NULL || actual_mac == NULL ||
        mac_len == 0 || ssl_export_client_write_mac_len == 0 ||
        mac_len != ssl_export_client_write_mac_len) {
        return;
    }

    if (mac_len == 20) {
        md_info = mbedtls_md_info_from_type(MBEDTLS_MD_SHA1);
    } else if (mac_len == 32) {
        md_info = mbedtls_md_info_from_type(MBEDTLS_MD_SHA256);
    } else if (mac_len == 48) {
        md_info = mbedtls_md_info_from_type(MBEDTLS_MD_SHA384);
    } else {
        return;
    }
    if (md_info == NULL) {
        return;
    }

    mbedtls_md_init(&md_ctx);
    if (mbedtls_md_setup(&md_ctx, md_info, 1) != 0) {
        mbedtls_md_free(&md_ctx);
        return;
    }
    if (mbedtls_md_hmac_starts(&md_ctx, ssl_export_client_write_mac, ssl_export_client_write_mac_len) != 0 ||
        mbedtls_md_hmac_update(&md_ctx, add_data, add_data_len) != 0 ||
        mbedtls_md_hmac_update(&md_ctx, plain, plain_len) != 0 ||
        mbedtls_md_hmac_finish(&md_ctx, expected) != 0) {
        mbedtls_md_free(&md_ctx);
        return;
    }
    mbedtls_md_free(&md_ctx);

    expected_hash = minios_fnv1a64(expected, mac_len);
    actual_hash = minios_fnv1a64(actual_mac, mac_len);
    ssl_last_expected_mac_hash = expected_hash;
    ssl_last_actual_mac_hash = actual_hash;
    ssl_last_mac_match = memcmp(expected, actual_mac, mac_len) == 0;
}

int minios_tls_last_out_record_decrypt_ok(void) {
    return ssl_last_out_record_decrypt_ok;
}

uint64_t minios_tls_last_out_record_plaintext_hash(void) {
    return ssl_last_out_record_plaintext_hash;
}

uint32_t minios_tls_last_out_record_plaintext_len(void) {
    return ssl_last_out_record_plaintext_len;
}

uint32_t minios_tls_last_out_record_padlen(void) {
    return ssl_last_out_record_padlen;
}

int minios_tls_last_cbc_reencrypt_match(void) {
    return ssl_last_cbc_reencrypt_match;
}

uint64_t minios_tls_last_cbc_plain_hash(void) {
    return ssl_last_cbc_plain_hash;
}

uint64_t minios_tls_last_cbc_expected_cipher_hash(void) {
    return ssl_last_cbc_expected_cipher_hash;
}

uint64_t minios_tls_last_cbc_actual_cipher_hash(void) {
    return ssl_last_cbc_actual_cipher_hash;
}

uint32_t minios_tls_last_cbc_len(void) {
    return ssl_last_cbc_len;
}

int minios_tls_last_mac_match(void) {
    return ssl_last_mac_match;
}

uint64_t minios_tls_last_expected_mac_hash(void) {
    return ssl_last_expected_mac_hash;
}

uint64_t minios_tls_last_actual_mac_hash(void) {
    return ssl_last_actual_mac_hash;
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
    ssl_export_count = 0;
    ssl_export_client_random_hash = 0;
    ssl_export_server_random_hash = 0;
    ssl_export_master_hash = 0;
    ssl_export_keyblock_hash = 0;
    ssl_export_client_random_prefix = 0;
    ssl_export_server_random_prefix = 0;
    ssl_export_maclen = 0;
    ssl_export_keylen = 0;
    ssl_export_ivlen = 0;
    ssl_export_prf_type = 0;
    ssl_export_client_write_key_hash = 0;
    ssl_export_client_write_mac_hash = 0;
    ssl_export_server_write_key_hash = 0;
    ssl_export_client_write_key_aes_zero_hash = 0;
    ssl_export_client_write_key_aes_zero_hash_static = 0;
    ssl_export_client_write_key_prefix = 0;
    ssl_export_client_write_mac_len = 0;
    ssl_export_client_write_key_len = 0;
    ssl_last_out_record_decrypt_ok = 0;
    ssl_last_out_record_plaintext_hash = 0;
    ssl_last_out_record_plaintext_len = 0;
    ssl_last_out_record_padlen = 0;
    ssl_last_cbc_reencrypt_match = 0;
    ssl_last_cbc_plain_hash = 0;
    ssl_last_cbc_expected_cipher_hash = 0;
    ssl_last_cbc_actual_cipher_hash = 0;
    ssl_last_cbc_len = 0;
    ssl_last_mac_match = 0;
    ssl_last_expected_mac_hash = 0;
    ssl_last_actual_mac_hash = 0;
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
