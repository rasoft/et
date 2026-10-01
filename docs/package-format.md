# etpk 包格式

版本：1

本文是第一期实现要遵守的格式说明。编辑器的工作副本是目录。zip 导出是以后的分发形式，解压后必须得到本文描述的目录。

## 1. 目录

```
<name>.etpk/
  manifest.json
  images/
    <partition-id>.img      # 有镜像的分区才有
    .trash/                 # 运行时撤销用，可以不存在
```

- 包根目录名建议以 `.etpk` 结尾。打开时以存在 `manifest.json` 为准，不以扩展名为准。
- `images/` 在新建时就创建。没有分区镜像时目录可以为空。
- `images/.trash/` 是应用私有目录。打开时若存在，核心库不把它当成分区镜像。它不应该出现在手工整理后的分发包里。
- 除 `manifest.json` 和 `images/` 以外的文件，第一期忽略，保存时原样留下。

## 2. manifest.json

UTF-8，无 BOM。字段名使用 camelCase。未知字段在同一主版本内保留并在下次保存时原样写回，避免新工具写过的可选字段被旧工具抹掉。第一期实现可以拒绝带未知字段的文件，也可以保留；若保留，必须有测试。推荐保留。

整数都是 JSON number，且必须能被无符号 64 位整数精确表示，序列化时写十进制整数字面量，不用科学计数法。容量和偏移用字节，不用 LBA，避免界面层再乘一次扇区。

### 2.1 顶层

```json
{
  "format": "etpack",
  "formatVersion": 1,
  "metadata": {},
  "partitions": []
}
```

| 字段 | 类型 | 约束 |
| --- | --- | --- |
| `format` | string | 必须为 `etpack` |
| `formatVersion` | number | 必须为整数 `1`。其他值拒绝打开 |
| `metadata` | object | 见下 |
| `partitions` | array | 顺序即自动排布顺序。长度 0 到 128 |

不使用 JSON Schema 文件作为运行时依赖。本文的表格就是约束。若以后要加 schema，从本文生成，不另写一套规则。

### 2.2 metadata

```json
{
  "name": "board-d1",
  "description": "",
  "sectorSize": 512,
  "userAreaBytes": 17179869184,
  "alignment": 1048576
}
```

| 字段 | 类型 | 约束 |
| --- | --- | --- |
| `name` | string | 非空，最长 128 个 Unicode 标量值。是包的显示名，不是目录名 |
| `description` | string | 可为空，最长 4096 个标量值 |
| `sectorSize` | number | `512` 或 `4096` |
| `userAreaBytes` | number | 大于 0，且是 `sectorSize` 的整数倍。必须大于主备 GPT 保留区之和 |
| `alignment` | number | 大于 0，且同时是 `sectorSize` 和 512 的整数倍。默认 `1048576`（1 MiB） |

第一期新建包时写入上述全部字段。以下字段第一期可以不出现；出现时必须合法，打开后原样保留，界面不编辑：

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `boot1Bytes` | number | Boot1 容量，字节 |
| `boot2Bytes` | number | Boot2 容量，字节 |
| `boot1Image` | string 或 null | 相对路径，规则同分区镜像 |
| `boot2Image` | string 或 null | 同上 |

不写创建时间。文件系统自己有时间戳；写入 `createdAt` 会让每次另存为都产生无意义差异。需要版本记录时用目录副本或 git，不塞进格式。

### 2.3 分区对象

```json
{
  "id": "6f1c2a0e-7b4d-4e3a-9c1f-2a8b0d5e6f70",
  "name": "boot",
  "sizeBytes": 67108864,
  "startBytes": null,
  "type": "linux-filesystem",
  "attributes": 0,
  "image": "images/6f1c2a0e-7b4d-4e3a-9c1f-2a8b0d5e6f70.img"
}
```

| 字段 | 类型 | 约束 |
| --- | --- | --- |
| `id` | string | UUID，RFC 4122 的文本形式，小写。包内唯一 |
| `name` | string | 包内唯一。非空。只含 `A-Z a-z 0-9 - _ .`。按 UTF-16 码元计最长 36（在本字符集下等价于 36 个字符） |
| `sizeBytes` | number | 大于 0，且是 `sectorSize` 的整数倍 |
| `startBytes` | number 或 null | `null` 表示自动排布。数字表示固定起点，从用户区字节 0 算起，必须是 `sectorSize` 的整数倍 |
| `type` | string | 预设名或 `guid:<36字符 GUID>`。第一期写入 `linux-filesystem` |
| `attributes` | number | 0 到 2^64-1 的整数。第一期写入 `0`。对应 GPT 属性位 |
| `image` | string 或 null | `null` 表示没有镜像。字符串必须是 `images/<id>.img`，其中 `<id>` 与该分区 `id` 相同 |

`startBytes` 用 JSON `null`，不要省略字段。省略和 `null` 在第一期都按自动排布解释；保存时始终写出 `null` 或数字。

## 3. 预设类型

`type` 为下表左列时，生成 GPT 时使用右列的类型 GUID。第一期只要求把字符串存进 manifest，不要求写出 GPT。

| 预设名 | GUID |
| --- | --- |
| `linux-filesystem` | `0FC63DAF-8483-4772-8E79-3D69D8477DE4` |
| `efi-system` | `C12A7328-F81F-11D2-BA4B-00A0C93EC93B` |
| `bios-boot` | `21686148-6449-6E6F-744E-656564454649` |

自定义类型写作 `guid:0fc63daf-8483-4772-8e79-3d69d8477de4`（小写）。不认识的预设名：打开成功，校验时对该分区给出错误，避免静默落成错误的 GPT 类型。第一期界面不提供修改入口，因此新建分区不会产生不认识的类型。

## 4. 布局算法

输入：`sectorSize`、`userAreaBytes`、`alignment`、按数组顺序排列的分区。

常量：

- `primaryReserved = 34 * sectorSize`
- `backupReserved = 33 * sectorSize`
- 可用半开区间 `[primaryReserved, userAreaBytes - backupReserved)`

若 `userAreaBytes - backupReserved <= primaryReserved`，整份文档无效，拒绝打开。

对每条分区按顺序维护 `cursor`，初始为 `primaryReserved`：

1. 若 `startBytes` 为 `null`：`start = align_up(cursor, alignment)`。
2. 若 `startBytes` 为数字：`start = startBytes`，并且不修改后续计算对该分区的“占用”以外的特殊规则；`cursor` 仍要前进，见第 4 步。
3. `end = start + sizeBytes`。
4. `cursor = max(cursor, end)`。下一条自动分区从新的 cursor 对齐。固定分区若留在后面自动分区的前面，不把 cursor 拉回。

`align_up(value, alignment)`：`value` 已对齐时返回 `value`，否则返回向上补齐的结果。用无符号整数，注意加法溢出时视为该分区超出容量。

校验在算出全部 `start`/`end` 之后进行：

- `start >= primaryReserved`
- `end <= userAreaBytes - backupReserved`
- 任意两条 `[start, end)` 不相交
- 自动分区的 `start` 是 `alignment` 的整数倍
- 镜像文件长度 `<= sizeBytes`（文件不存在则另报缺失，不做大小比较）

示例：16 GiB 用户区，512 字节扇区，1 MiB 对齐。`primaryReserved` 为 17408。第一条自动分区起点对齐到 1048576（1 MiB）。这是预期结果，不是把第一条放在 LBA 34。

## 5. 镜像文件

- 内容原样保存，不压缩、不转换成 sparse、不补齐到分区大小。
- 文件名只由分区 id 决定，所以重命名分区不会改文件名，撤销重命名也不用动文件。
- 替换镜像时写入 `images/<id>.img.partial`，完成后在同一目录内改名为 `images/<id>.img`。
- 不跟随符号链接去读包外文件。复制源可以是用户选定的包外常规文件；写入目标必须是包内的新文件。打开包做校验时，`image` 路径解析后的真实路径必须仍在包根之内。

## 6. 版本演进

- `formatVersion` 目前是整数。不兼容的删除字段或改变布局算法含义时加 1，旧程序拒绝打开。
- 只增加可选字段且默认行为不变时，保持 `formatVersion` 为 1，旧程序按第 2 节保留未知字段。
- 不在第一期实现版本迁移器。
