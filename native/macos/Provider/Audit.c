#include <bsm/libbsm.h>
#include <stdint.h>
#include <string.h>
uint32_t drift_audit_euid(const void *data, size_t length) {
    if (!data || length != sizeof(audit_token_t)) return UINT32_MAX;
    audit_token_t token;
    memcpy(&token, data, sizeof(token));
    return audit_token_to_euid(token);
}
int32_t drift_audit_pid(const void *data, size_t length) {
    if (!data || length != sizeof(audit_token_t)) return -1;
    audit_token_t token;
    memcpy(&token, data, sizeof(token));
    return audit_token_to_pid(token);
}
