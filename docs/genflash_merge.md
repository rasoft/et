# genflash merge 输出文件格式

命令：

```sh
genflash merge flash.config sth.bin
```

对应实现：`cmd_merge()` → `flash_load_conf()` → `image_merge()`（`main.c`、`flash.c`）。

本命令不带 `-b`，产物是纯 merge 包。文件在最后一个子文件数据处结束，末尾没有 host `.boot`，也没有 `GXBf` 尾。

## 输出文件名

`sth.bin` 的文件名里已经有扩展名，输出路径就是 `sth.bin`，不会再追加 `.merge`。

规则（`resolve_merge_output_name`）：

- 文件名含 `.` 且点号后面还有字符（`.bin`、`.merge` 等）→ 按给定路径原样写出。
- 文件名没有扩展名 → 追加 `.merge`。

## 整体布局

各段首尾相接，中间没有对齐填充，也没有按分区地址留出的空洞。

```
偏移 0
+------------------------------------------+
| merge_head          12 字节              |
+------------------------------------------+
| merge_sub_file[0]   272 字节             |  索引区，共 N 条
| merge_sub_file[1]   272 字节             |
| ...                                      |
| merge_sub_file[N-1] 272 字节             |
+------------------------------------------+  偏移 = 12 + N * 272
| 子文件 0 的数据     len[0] 字节          |  永远是 flash.config 原文
| 子文件 1 的数据     len[1] 字节          |
| ...                                      |
+------------------------------------------+  文件在此结束
```

```
文件大小 = 12 + N * 272 + sum(len[i])
```

`N = sub_file_num`，至少为 1（只有配置文件本身）。

索引里的 `start_addr` 是该子文件数据在本文件中的绝对偏移，不是 flash 上的烧写地址。第一条数据的 `start_addr` 等于 `12 + N * 272`，之后每条的 `start_addr` 等于上一条的 `start_addr + len`。

## 字节序与对齐

文件头和索引按宿主 C 结构体内存原样 `fwrite`，没有单独做字节序转换。在 Linux x86_64 / aarch64 上为小端：

| 结构 | sizeof | 说明 |
| --- | ---: | --- |
| `merge_head_s` | 12 | `magic` 在 0，`sub_file_num` 在 8 |
| `merge_sub_file_s` | 272 | `name` 256 字节，`start_addr` 在记录内偏移 256，`len` 在 264 |

索引从文件偏移 12 开始。`272` 能被 8 整除，所以每一条记录的起始偏移都是 `12 + i * 272`，对 8 取模为 4。记录内的两个 `uint64` 因此只保证 4 字节对齐。解析时应按字节读取，不要把映射地址直接当成自然对齐的 `uint64` 去解引用。

分区表二进制（下面的 TABLE 子文件）内部的 16/32 位字段是另一套规则：显式大端（`int16tochar` / `int32tochar`）。

## 文件头 `merge_head_s`（12 字节）

| 偏移 | 长度 | 字段 | 内容 |
| ---: | ---: | --- | --- |
| 0 | 8 | `magic` | ASCII `mergebin`。定长 8 字节，没有结尾 NUL |
| 8 | 4 | `sub_file_num` | `int32`，宿主字节序。等于索引条数 |

`split` 用这 8 字节是否等于 `mergebin` 判断是不是 merge 文件。

## 索引项 `merge_sub_file_s`（每条 272 字节）

| 记录内偏移 | 长度 | 字段 | 内容 |
| ---: | ---: | --- | --- |
| 0 | 256 | `name` | 以 NUL 结尾的文件名，不足部分为 0。只保存 basename，不含目录 |
| 256 | 8 | `start_addr` | `uint64`，宿主字节序。数据在本文件中的起始偏移 |
| 264 | 8 | `len` | `uint64`，宿主字节序。数据字节数 |

`name` 是源文件名，不是分区名。两个分区如果引用同一个文件（例如都写 `loader-sflash.bin`），索引里会出现两条同名记录，数据各写一份。`split` 会把它们写到同一路径，后写的覆盖先写的。

## 子文件顺序与内容

`sub_file_num` 的计算：先计 1（配置文件），再为每个 `file_name != NULL` 的分区加 1。

写入顺序：

1. **第 0 条永远是配置文件。**  
   `name` 为路径的 basename（本命令是 `flash.config`）。数据是该文件的原始字节，长度等于文件大小。注释、空行都原样保留。

2. **其后是有镜像文件的分区，按 `start_addr` 升序。**  
   `flash_load_conf()` 末尾会 `flash_sort()`，比较的是分区起始地址，不是配置文件里的书写顺序。

纳入规则：

- 配置里 `FILE` 列不是 `NULL` 的分区会进入包。数据是源文件内容（相对配置文件所在目录打开）。
- `FILE` 为 `NULL`、且分区名不是 `TABLE` 的分区不进入包。它的地址、大小仍写在配置文本和 TABLE 分区表里，只是没有对应的数据段。
- 名为 `TABLE` 的分区总会进入包。`image_merge()` 按 `table_version` 现场生成分区表，再当作一个子文件写入。默认文件名是 `table.bin`（写在配置文件所在目录）。若 `FILE` 列给了非 `NULL` 的名字，索引用这个名字，内容仍是生成的表，不是磁盘上那个原文件。

普通分区（`table_type` 不是 `gpt`）写入长度是 `used_size`，单位是字节，等于源文件大小。分区剩余空间在内存里会被填成 `0xFF` 用来算表，但这部分**不会**写入 merge 文件。merge 包不是按 flash 地址铺开的整片镜像，烧写工具要另按配置或分区表把各段放到对应地址。

`table_type gpt` 时，配置里的尺寸以 block 为单位。`used_size` 会被向上取整到 block，写入长度是 `used_size * block_size`。

若配置里有 `dtb_file`，且分区名是 `KERNEL`，该段数据是 `dtb 文件 || kernel 文件`，`used_size` 为两者之和。索引名仍是 KERNEL 那一列的源文件名。

`zlibmode` 只影响 `mkflash` 写出的 flash 镜像，不改变本命令的容器格式。merge 写入的仍是未压缩的源文件字节。

## TABLE 子文件

固定长度由 `table_version` 决定。未使用区域先填 `0xFF`，多字节整数为大端。表内包含配置中的全部分区（含 `FILE` 为 `NULL`、因而没有数据段的分区）。

| `table_version` | magic（大端） | 长度 | 最多分区数 |
| ---: | --- | ---: | ---: |
| 1（缺省） | `0xAABCDEFA` | 512 | 12 |
| 2 | `0xDD1C2BFA` | 1024 | 24 |
| 4 | `0xFF3E4D1C` | 3072 | 84 |
| 5 | `0xCCABEF12` | 8192 | 84 |

公共骨架（v1 / v2 / v4 的分区项为 24 或 28 字节，v5 为 84 字节；细节见 `flash.h` 里的分区表图和 `flash_table*_write()`）：

| 区域 | 内容 |
| --- | --- |
| 0..3 | magic，大端 |
| 4 | 分区个数 `count` |
| 紧随其后的分区项数组 | 名字、`total_size`、`used_size`、（v2 起）`reserved_size`、`start_addr`、文件系统类型、mode / crc 使能、id / 写保护、update |
| version 表 | 每分区一个大端 `uint16` |
| crc 表 | 每分区一个大端 `uint32`（该分区镜像的 zlib crc32）。未使用槽位为 `0xFFFFFFFF` |
| 末尾 7 字节 | `write_protect`、`crc32_enable`、分区表版本字节、4 字节大端表 crc。表 crc 覆盖除这 4 字节以外的全部表内容 |

这里的 `start_addr` / `total_size` 是 flash 上的分区位置和容量。索引区里的 `start_addr` / `len` 是 merge 文件内的偏移和数据长度，两者不是同一套地址。

## 解析步骤

1. 读偏移 0 的 8 字节，确认为 `mergebin`。
2. 读偏移 8 的小端 `int32`，得到 `N`。
3. 从偏移 12 起读 `N` 条 272 字节记录。每条：256 字节文件名（读到第一个 NUL），随后两个小端 `uint64`（`start_addr`、`len`）。
4. 用 `start_addr` 和 `len` 切出数据。第 0 段是 `flash.config` 原文；名为 `table.bin`（或配置里为 TABLE 指定的文件名）的那一段是分区表；其余段是对应源文件的原始字节。
5. 本命令的文件应正好在最后一段数据处结束。若文件更长且末尾 64 字节为 `GXBf`，那是带 `-b` 的扩展包，见下一节。

## 带 `-b` 时多出来的尾部

`genflash merge -b host.boot flash.config sth.bin` 会在纯 merge 之后再追加两段。不带 `-b` 的 `sth.bin` 没有这两段。`split` 能识别尾部，但索引区的 `sub_file_num` 不包含 `.boot`。

```
[纯 merge，长度 = original_image_size]
[host .boot，长度 = boot_size]
[MergeBootFooter，固定 64 字节]
```

Footer 为显式小端，与文件头的“按宿主结构体原样写”不同：

| 偏移 | 长度 | 字段 | v1.0 的值 |
| ---: | ---: | --- | --- |
| 0 | 4 | magic | ASCII `GXBf` |
| 4 | 4 | `version_major` | 1 |
| 8 | 4 | `version_minor` | 0 |
| 12 | 8 | `original_image_size` | 纯 merge 的字节数 |
| 20 | 8 | `boot_offset` | 与 `original_image_size` 相等 |
| 28 | 8 | `boot_size` | `.boot` 字节数，1 .. 32MiB |
| 36 | 4 | `boot_crc32` | `.boot` 内容的 zlib crc32，必须校验 |
| 40 | 24 | reserved | 全 0 |

合法时：`original_image_size + boot_size + 64` 等于整个文件大小。`.boot` 开头 4 字节小端 magic 为 `0x626F6F74`（ASCII `boot`）。
