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

/* DocumentView JSON 的字段：
 * root、dirty、canUndo、canRedo、
 * metadata.name / description / sectorSize / userAreaBytes / alignment、
 * partitions、issues。
 * 整数是十进制 JSON number。
 *
 * 新建包并作为当前文档。dir_utf8 和 name_utf8 是以 NUL 结尾的 UTF-8。
 * user_area_bytes 是用户区字节数。sector_size 只能是 512 或 4096。
 * discard_unsaved 非 0 时，若当前包有未保存修改，先丢掉内存里的修改再新建。
 * 成功返回 0，*out_view 是 DocumentView。失败返回 1，*out_error 是说明。
 * 两个出参必须非空；函数会先把它们置为 NULL。调用方用 et_string_free 交还。
 */
int32_t et_create_package(
    const char *dir_utf8,
    const char *name_utf8,
    uint64_t user_area_bytes,
    uint32_t sector_size,
    int32_t discard_unsaved,
    char **out_view,
    char **out_error);

#ifdef __cplusplus
}
#endif

#endif
