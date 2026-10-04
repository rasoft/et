```
flash_type    emmc
block_size    0x200
#flash_size    0x500000
table_type    gpt
#write_protect true
crc32         true
table_version 5
#dtb_file     generic.dtb

# 1) NAME field:
#   "BOOT"   : gxloader
#   "TABLE"  : partition table
#   "LOGO"   : gxloader show logo
#   "KERNEL" : application program
#   "ROOT"   : root file system
#
# 2) The FS field have these types:
#           "RAW": self definition file system type;
#           "CRAMFS":  cram_file_system type;
#           "MINIFS":  mini_file_system type;
#
# 3) The partition have 2 mode "ro" and "rw":
#           "ro": means that this partition could not been modified at runtime;
#           "rw": means that this partition could modify at runtime
# 4) UPDATE:
#     0: don't update
#     1: always update
#     2: auto update, while version > old version
#     3: clear partition data
#
# 5) VERSION 0 - 65535 (0x0000 - 0xFFFF)
#
# 6) The size of "auto" means the size of partition is determined by source file size, but for
#    the final partition (example "DATA") it means the size is last to the end of flash.
#
## NOTE:
#
# 1) The size of source file could not exceed the max size of partition.
# 2) The start addr of "BOOT" and "LOGO" should never be modified whenever you are
#    modifying this file (configure file) or running a program.
# 3) The 1K bytes blank befor the "LOGO" partition is reserved for partition table,
#    so do not use these space.
#
# NAME              FILE                   CRC     FS       MODE    UPDATE VERSION  ADDRESS   SIZE
#-------------------------------------------------------------------------------------------
security          null.img               true    RAW      ro      0      0        0x2000     0x2000
uboot             null.img               true    RAW      ro      0      0        0x4000     0x2000
trust             null.img               true    RAW      ro      0      0        0x6000     0x2000
misc              misc.img               true    RAW      ro      1      0        0x8000     0x2000
dtbo_a            dtbo.img               true    RAW      ro      1      0        0xa000     0x2000
dtbo_b            null.img               true    RAW      ro      0      0        0xc000     0x2000
vbmeta_a          vbmeta.img             true    RAW      ro      1      0        0xe000     0x800
vbmeta_b          null.img               true    RAW      ro      0      0        0xe800     0x800
vbmeta_system_a   vbmeta_system.img      true    RAW      ro      1      0        0xf000     0x800
vbmeta_system_b   null.img               true    RAW      ro      0      0        0xf800     0x800
boot_a            boot.img               true    RAW      ro      1      0        0x10000    0x20000
boot_b            null.img               true    RAW      ro      0      0        0x30000    0x20000
vendor_boot_a     vendor_boot.img        true    RAW      ro      1      0        0x50000    0x20000
vendor_boot_b     null.img               true    RAW      ro      0      0        0x70000    0x20000
init_boot_a       init_boot.img          true    RAW      ro      1      0        0x90000    0x8000
init_boot_b       null.img               true    RAW      ro      0      0        0x98000    0x8000
backup            null.img               true    RAW      ro      0      0        0xa0000    0xc0000
cache             null.img               true    RAW      ro      0      0        0x160000   0xc0000
metadata          null.img               true    RAW      ro      0      0        0x220000   0x20000
baseparameter     baseparameter.img      true    RAW      ro      1      0        0x240000   0x800
super             super.img              true    RAW      ro      1      0        0x240800   0x600000
frp               null.img               true    RAW      rw      0      0        0x840800   0x800
userdata          null.img               true    RAW      ro      0      0        0x841000   auto

```