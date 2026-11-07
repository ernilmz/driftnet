#ifndef DRIFT_POLICY_H
#define DRIFT_POLICY_H
#include <stddef.h>
#include <stdint.h>
void *drift_policy_create(const uint8_t *data, size_t length);
void drift_policy_destroy(void *policy);
uint8_t drift_policy_selected(const void *policy, const uint8_t *identity, size_t identity_length, const uint8_t *hostname, size_t hostname_length);
uint32_t drift_audit_euid(const void *audit_data, size_t length);
#endif
int32_t drift_audit_pid(const void *audit_data, size_t length);
