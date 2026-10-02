#ifndef ET_ABI_H
#define ET_ABI_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

/* Qt 与 Rust 之间只通过这个头通信。后续的 DocumentView 是 UTF-8 JSON，
 * 由 Rust 分配，用 et_string_free 交还。界面不在 C++ 里计算分区布局。
 */

/* 当前为 1。界面可据此拒绝过旧的命令层。 */
uint32_t et_abi_version(void);

/* 进程存活期间有效，不要传给 et_string_free。 */
const char *et_version(void);

/* 释放命令层分配并交给界面的字符串。空指针合法。 */
void et_string_free(char *text);

#ifdef __cplusplus
}
#endif

#endif
